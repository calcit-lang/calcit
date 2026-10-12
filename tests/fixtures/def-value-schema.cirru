
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.binding-proof $ %{} 'FileEntry
      :defs $ {}
        'Box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Box
            :value $ :: 'Optional 'app.binding-proof/Value
          :examples $ []
          :schema $ :: 'StructDef
        'OpenBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OpenBox (:value 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'OptionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptionBox
            :value $ :: 'Optional $ :: 'Option 'String
          :examples $ []
          :schema $ :: 'StructDef
        'Value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Value (:text 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'choose-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose-nil (present)
            let
                selected $ if present nil nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert= (Box :value nil) (choose-nil true)
              assert= (Box :value nil) (choose-nil false)
            :tags $ #{} :nullable-branch-binding
        'choose-nullable $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose-nullable (present box)
            let
                selected $ if present (:value box) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool 'app.binding-proof/Box
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert= (Box :value nil)
                choose-nullable false $ Box :value $ Value :text |ok
              assert= (Box :value nil)
                choose-nullable true $ Box :value nil
              assert=
                Box :value $ Value :text |ok
                choose-nullable true $ Box :value $ Value :text |ok
            :tags $ #{} :nullable-branch-binding
        'choose-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose-option (present value)
            let
                selected $ if present value nil
              OptionBox :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/OptionBox)
            :args $ [] 'Bool $ :: 'Option 'String
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert= (OptionBox :value nil)
                choose-option false $ Option :some |ok
              assert=
                OptionBox :value $ Option :some |ok
                choose-option true $ Option :some |ok
              assert=
                OptionBox :value $ Option :none
                choose-option true $ Option :none
            :tags $ #{} :nullable-branch-binding
        'decode-lookup $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-lookup (input)
            let
                selected $ match (input .get |key)
                  (:some raw)
                    match (try-decode-map-as raw app.binding-proof/Value)
                      (:ok typed) typed
                      (:err reason) nil
                  (:none) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Map 'String 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                decode-lookup $ {} $ |key
                  {} $ :text |ok
              assert= (Box :value nil)
                decode-lookup $ {} $ |key 42
              assert= (Box :value nil)
                decode-lookup $ {} $ |other 42
            :tags $ #{} :open-match-payload
        'decode-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-option (input)
            let
                selected $ match input
                  (:some raw)
                    match (try-decode-map-as raw app.binding-proof/Value)
                      (:ok typed) typed
                      (:err reason) nil
                  (:none) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                decode-option $ Option :some $ {} (:text |ok)
              assert= (Box :value nil)
                decode-option $ Option :some 42
              assert= (Box :value nil)
                decode-option $ Option :none
            :tags $ #{} :open-match-payload
        'decode-result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-result (input)
            let
                selected $ match input
                  (:ok raw)
                    match (try-decode-map-as raw app.binding-proof/Value)
                      (:ok typed) typed
                      (:err reason) nil
                  (:err reason) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Result 'Dynamic 'String
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                decode-result $ Result :ok $ {} (:text |ok)
              assert= (Box :value nil)
                decode-result $ Result :ok 42
              assert= (Box :value nil)
                decode-result $ Result :err |missing
            :tags $ #{} :open-match-payload
        'if-nil-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-nil-first (present)
            let
                selected $ if present nil $ Value :text |ok
                alias selected
              Box :value alias
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                if-nil-first false
              assert= (Box :value nil) (if-nil-first true)
            :tags $ #{} :nullable-branch-binding
        'if-value-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-value-first (present)
            let
                selected $ if present (Value :text |ok) nil
                alias selected
              Box :value alias
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-branches)
            :code $ quote $ do
              assert=
                Box :value $ Value :text |ok
                if-value-first true
              assert= (Box :value nil) (if-value-first false)
            :tags $ #{} :nullable-branch-binding
        'lookup-inline $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lookup-inline (items)
            Box :value $ match (items .get |key)
              (:some value) value
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Map 'String 'app.binding-proof/Value
          :tests $ [] $ %{} 'TestEntry (:name |present-and-missing)
            :code $ quote $ do
              assert= (Box :value nil)
                lookup-inline $ {} $ |other (Value :text |ok)
              assert=
                Box :value $ Value :text |ok
                lookup-inline $ {} $ |key (Value :text |ok)
            :tags $ #{} :nullable-branch-binding
        'lookup-local $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lookup-local (items)
            let
                selected $ match (items .get |key)
                  (:some value) value
                  (:none) nil
              Box :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/Box)
            :args $ [] $ :: 'Map 'String 'app.binding-proof/Value
          :tests $ [] $ %{} 'TestEntry (:name |present-and-missing)
            :code $ quote $ do
              assert= (Box :value nil)
                lookup-local $ {} $ |other (Value :text |ok)
              assert=
                Box :value $ Value :text |ok
                lookup-local $ {} $ |key (Value :text |ok)
            :tags $ #{} :nullable-branch-binding
        'retain-open $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn retain-open (input)
            let
                selected $ match input
                  (:some value) value
                  (:none) nil
              OpenBox :value selected
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.binding-proof/OpenBox)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |checked-and-open-payloads)
            :code $ quote $ do
              assert= (Result :ok 42)
                try-decode-map-as
                  :value $ retain-open $ Option :some 42
                  , 'Number
              assert= (Result :ok |text)
                try-decode-map-as
                  :value $ retain-open $ Option :some |text
                  , 'String
              assert= true $ nil? $ :value
                retain-open $ Option :none
            :tags $ #{} :open-match-payload
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.binding-proof
    'app.branch-call $ %{} 'FileEntry
      :defs $ {}
        'count-rest $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-rest (first-style & more) (count more)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :rest $ :: 'Map 'Tag 'Dynamic
        'count-style $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-style (style) (count style)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
        'open-style $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-style (raw)
            count-style $ if true raw $ {}
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'Map 'Dynamic 'Dynamic
        'rebound-style $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rebound-style (raw)
            count-style $ let
                style $ {}
              set! style raw
              , style
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
        'rest-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rest-count (flag)
            count-rest
              {} $ :padding |4px
              if flag
                {} $ :color |red
                {}
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |both-rest-exits)
            :code $ quote $ do
              assert= 1 $ rest-count true
              assert= 1 $ rest-count false
            :tags $ #{} :branch-call-proof
        'style-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn style-count (flag)
            count-style $ if flag
              {} $ :color |red
              {}
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |both-exits)
            :code $ quote $ do
              assert= 1 $ style-count true
              assert= 0 $ style-count false
            :tags $ #{} :branch-call-proof
        'wrong-style $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-style (flag)
            count-style $ if flag
              {} $ 1 |red
              {}
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.branch-call
    'app.callback-left $ %{} 'FileEntry
      :defs $ {} $ 'Item
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Item (:n 'Number)
          :examples $ []
          :schema $ :: 'StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.callback-left
    'app.callback-origin $ %{} 'FileEntry
      :defs $ {}
        'Controller $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Controller ([] 'Op)
            :dispatch $ :: 'Ref $ :: 'Fn
              {}
                :args $ [] 'Op
                :return 'Unit
          :examples $ []
          :schema $ :: 'StructDef
        'RepeatedPayload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum RepeatedPayload ([] 'T) (:pair 'T 'T)
          :examples $ []
          :schema $ :: 'EnumDef
        'branch-empty-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn branch-empty-generic-list (anchor flag open)
            assert-type
              if flag ([]) ([])
              :: 'List 'T
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] (:: 'List 'T) 'Bool $ :: 'List 'Dynamic
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |both-empty-exits)
            :code $ quote $ do
              assert= &unit $ branch-empty-generic-list ([] 1) true $ []
              assert= &unit $ branch-empty-generic-list ([] |text) false $ []
            :tags $ #{} :empty-generic-family
        'branch-nonempty-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn branch-nonempty-generic-list (anchor flag open)
            assert-type
              if flag ([]) ([] 1)
              :: 'List 'T
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] (:: 'List 'T) 'Bool $ :: 'List 'Dynamic
            :generics $ [] 'T
        'branch-open-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn branch-open-generic-list (anchor flag open)
            assert-type
              if flag ([]) open
              :: 'List 'T
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] (:: 'List 'T) 'Bool $ :: 'List 'Dynamic
            :generics $ [] 'T
        'choose $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn choose (flag)
            let
                original $ left/Item :n 1
              if-let
                present? $ if flag (Option :some true) (Option :none)
                struct-with original $ :n 2
                struct-with original $ :n 3
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.callback-left/Item)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry
            :name |preserves-nominal-origin-through-option-callbacks
            :code $ quote $ do
              assert= 2 $ :n $ choose true
              assert= 3 $ :n $ choose false
            :tags $ #{} :nominal-callback-origin
        'closed-heterogeneous-pairs $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn closed-heterogeneous-pairs ()
            number-map-count $ pairs-map $ [] ([] :a 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'closed-homogeneous-pairs $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn closed-homogeneous-pairs ()
            number-map-count $ pairs-map $ [] ([] 1 2) ([] 3 4)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |feeds-concrete-map-consumer)
            :code $ quote $ assert= 2 (closed-homogeneous-pairs)
            :tags $ #{} :pairs-map-proof
        'closed-wrong-member-pairs $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn closed-wrong-member-pairs ()
            number-map-count $ pairs-map $ [] ([] |key |value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'direct-controller $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn direct-controller ()
            let
                received $ atom 0
                dispatch! $ fn (op)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Unit
                  reset! received op
                  , &unit
                controller $ Controller :dispatch $ atom dispatch!
              assert-type controller $ :: Controller Number
              (deref (:dispatch controller))
                , 7
              deref received
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |infers-reference-callback)
              :code $ quote $ assert= 7 (direct-controller)
              :tags $ #{} :ref-callback-inference
            %{} 'TestEntry (:name |empty-nominal-callback-input)
              :code $ quote $ let
                  controller $ Controller :dispatch $ atom
                    fn (items)
                      hint-fn $ {}
                        :args $ [] $ :: List 'app.callback-left/Item
                        :return Unit
                      assert= 0 $ count items
                      , &unit
                assert= &unit $
                  deref $ :dispatch controller
                  []
              :tags $ #{} :ref-callback-inference
        'empty-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn empty-generic-list (anchor)
            assert-type ([]) (:: 'List 'T)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'List 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |preserves-empty-family)
            :code $ quote $ assert= &unit
              empty-generic-list $ [] 1
            :tags $ #{} :empty-generic-family
        'empty-generic-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn empty-generic-map (anchor)
            assert-type ({}) (:: 'Map 'K 'V)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'Map 'K 'V
            :generics $ [] 'K 'V
          :tests $ [] $ %{} 'TestEntry (:name |preserves-empty-family)
            :code $ quote $ assert= &unit
              empty-generic-map $ {} $ 1 2
            :tags $ #{} :empty-generic-family
        'empty-generic-set $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn empty-generic-set (anchor)
            assert-type (#{}) (:: 'Set 'T)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'Set 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |preserves-empty-family)
            :code $ quote $ assert= &unit
              empty-generic-set $ #{}
            :tags $ #{} :empty-generic-family
        'enum-claim-open $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn enum-claim-open (raw)
            let
                seed $ Result :ok raw
              assert-type seed $ :: 'Result 'Number 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :return $ :: 'Result 'Number 'String
        'enum-claim-wrong $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn enum-claim-wrong ()
            let
                seed $ Result :ok |wrong
              assert-type seed $ :: 'Result 'Number 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Result 'Number 'String
        'enum-inconsistent-pair $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn enum-inconsistent-pair (value)
            let
                pair $ RepeatedPayload :pair value 1
              assert-type pair $ :: 'app.callback-origin/RepeatedPayload 'T
              , pair
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'T
            :generics $ [] 'T
            :return $ :: 'app.callback-origin/RepeatedPayload 'T
        'enum-open-peer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn enum-open-peer (value raw)
            let
                pair $ RepeatedPayload :pair value raw
              assert-type pair $ :: 'app.callback-origin/RepeatedPayload 'T
              , pair
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'T 'Dynamic
            :generics $ [] 'T
            :return $ :: 'app.callback-origin/RepeatedPayload 'T
        'enum-specialize-caller $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn enum-specialize-caller (value)
            let
                seed $ Result :ok value
              assert-type seed $ :: 'Result 'Number 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'T
            :generics $ [] 'T
            :return $ :: 'Result 'Number 'String
        'fold-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn fold-generic-list (xs)
            foldl xs
              assert-type ([]) (:: 'List 'T)
              fn (acc item)
                hint-fn $ {}
                  :args $ [] (:: 'List 'T) 'T
                  :return $ :: 'List 'T
                append acc item
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'T
            :generics $ [] 'T
            :return $ :: 'List 'T
          :tests $ [] $ %{} 'TestEntry (:name |keeps-callback-and-accumulator-generic)
            :code $ quote $ do
              assert= ([] 1 2)
                fold-generic-list $ [] 1 2
              assert= ([])
                fold-generic-list $ []
            :tags $ #{} :empty-generic-family
        'generic-controller $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn generic-controller (dispatch! op)
            let
                controller $ Controller :dispatch $ atom dispatch!
              assert-type controller $ :: Controller 'T
              (deref (:dispatch controller))
                , op
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
              :: 'Fn $ {} (:return 'Unit)
                :args $ [] 'T
              , 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |keeps-lexical-type-variable)
            :code $ quote $ let
                received $ atom |before
              generic-controller
                fn (op)
                  hint-fn $ {}
                    :args $ [] 'String
                    :return 'Unit
                  reset! received op
                  , &unit
                , |after
              assert= |after $ deref received
            :tags $ #{} :ref-callback-inference
        'generic-pairs-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn generic-pairs-map (xs) (pairs-map xs)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'List 'T)
            :generics $ [] 'T
            :return $ :: 'Map 'T 'T
          :tests $ [] $ %{} 'TestEntry (:name |preserves-caller-member-variable)
            :code $ quote $ assert=
              {} (1 2) (3 4)
              generic-pairs-map $ [] ([] 1 2) ([] 3 4)
            :tags $ #{} :pairs-map-proof
        'mixed-origin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mixed-origin ()
            option:fold (Option :some true)
              fn () $ left/Item :n 1
              fn (present?) (right/Item :n 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.callback-left/Item)
            :args $ []
        'nested-literal-spread $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested-literal-spread ()
            &+ & $ [] 2 & $ [] 3
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-order-through-nested-literal-spreads)
            :code $ quote $ let
                sink $ fn (a b c)
                  hint-fn $ {}
                    :args $ [] 'Number 'Number 'Number
                    :return $ :: 'List 'Number
                  [] a b c
              assert= ([] 1 2 3)
                sink & ([] 1) & $ [] 2 & $ [] 3
              assert= 5 $ nested-literal-spread
            :tags $ #{} :nested-literal-spread
        'nested-open-spread $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested-open-spread (xs)
            &+ & $ [] 2 & xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
        'nested-wrong-member $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested-wrong-member ()
            &+ & $ [] 2 & $ [] |wrong
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'nonempty-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nonempty-generic-list (anchor)
            assert-type ([] 1) (:: 'List 'T)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'T
            :generics $ [] 'T
            :return $ :: 'List 'T
        'number-map-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn number-map-count (xs) (count xs)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'Map 'Number 'Number
        'open-generic-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-generic-list (anchor values)
            assert-type values $ :: 'List 'Number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'List 'Number) (:: 'List 'Dynamic)
            :return $ :: 'List 'Number
        'wrap-generic-error $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-generic-error (error)
            let
                seed $ Result :err error
              assert-type seed $ :: 'Result 'String $ :: 'List 'E
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'E
            :generics $ [] 'E
            :return $ :: 'Result 'String $ :: 'List 'E
          :tests $ [] $ %{} 'TestEntry (:name |preserves-error-slot-variable)
            :code $ quote $ assert=
              Result :err $ [] 1 2
              wrap-generic-error $ [] 1 2
            :tags $ #{} :enum-generic-payload
        'wrap-generic-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-generic-map (users)
            let
                seed $ Result :ok users
              assert-type seed $ :: 'Result (:: 'Map 'String 'T) 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'String 'T
            :generics $ [] 'T
            :return $ :: 'Result (:: 'Map 'String 'T) 'String
          :tests $ [] $ %{} 'TestEntry (:name |preserves-nested-caller-variable)
            :code $ quote $ assert=
              Result :ok $ {} $ |a 1
              wrap-generic-map $ {} $ |a 1
            :tags $ #{} :enum-generic-payload
        'wrap-generic-pair $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-generic-pair (left right)
            let
                pair $ RepeatedPayload :pair left right
              assert-type pair $ :: 'app.callback-origin/RepeatedPayload 'T
              , pair
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'T 'T
            :generics $ [] 'T
            :return $ :: 'app.callback-origin/RepeatedPayload 'T
          :tests $ [] $ %{} 'TestEntry (:name |repeated-payload-shares-caller-variable)
            :code $ quote $ assert= (RepeatedPayload :pair 1 2) (wrap-generic-pair 1 2)
            :tags $ #{} :enum-generic-payload
        'wrap-generic-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-generic-value (value)
            let
                seed $ Result :ok value
              assert-type seed $ :: 'Result 'T 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'T
            :generics $ [] 'T
            :return $ :: 'Result 'T 'String
          :tests $ [] $ %{} 'TestEntry (:name |preserves-direct-caller-variable)
            :code $ quote $ assert= (Result :ok |value) (wrap-generic-value |value)
            :tags $ #{} :enum-generic-payload
        'wrap-renamed-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-renamed-map (users)
            let
                seed $ Result :ok users
              assert-type seed $ :: 'Result (:: 'Map 'String 'U) 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'String 'U
            :generics $ [] 'U
            :return $ :: 'Result (:: 'Map 'String 'U) 'String
          :tests $ [] $ %{} 'TestEntry (:name |generic-renaming-is-equivalent)
            :code $ quote $ assert=
              Result :ok $ {} $ |a |value
              wrap-renamed-map $ {} $ |a |value
            :tags $ #{} :enum-generic-payload
        'wrong-controller-call $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-controller-call ()
            let
                dispatch! $ fn (op)
                  hint-fn $ {}
                    :args $ [] Number
                    :return Unit
                  , &unit
                controller $ Controller :dispatch $ atom dispatch!
              (deref (:dispatch controller))
                , |wrong
              , &unit
          :examples $ []
        'wrong-controller-write $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-controller-write ()
            let
                dispatch! $ fn (op)
                  hint-fn $ {}
                    :args $ [] Number
                    :return Unit
                  , &unit
                controller $ Controller :dispatch $ atom dispatch!
              reset! (:dispatch controller)
                fn (op)
                  hint-fn $ {}
                    :args $ [] String
                    :return Unit
                  , &unit
              , &unit
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.callback-origin
          :require (app.callback-left :as left) (app.callback-right :as right)
    'app.callback-right $ %{} 'FileEntry
      :defs $ {} $ 'Item
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Item (:n 'Number)
          :examples $ []
          :schema $ :: 'StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.callback-right
    'app.callback-shape $ %{} 'FileEntry
      :defs $ {}
        'group-open-rows $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn group-open-rows (rows)
            group-by (&map:vals rows)
              fn (row) (&map:get row :finished-time)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'String (:: 'Map 'Tag 'Dynamic)
            :return $ :: 'Map 'Dynamic $ :: 'List (:: 'Map 'Tag 'Dynamic)
          :tests $ [] $ %{} 'TestEntry (:name |known-map-open-members)
            :code $ quote $ let
                row $ {} (:finished-time 7) (:title |task)
              assert=
                {} $ 7 $ [] row
                group-open-rows $ {} $ |one row
            :tags $ #{} :callback-container-shape :unit
        'group-open-rows-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn group-open-rows-alias (rows)
            let
                group-items group-by
              group-items (&map:vals rows)
                fn (row) (&map:get row :finished-time)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'String (:: 'Map 'Tag 'Dynamic)
            :return $ :: 'Map 'Dynamic $ :: 'List (:: 'Map 'Tag 'Dynamic)
          :tests $ [] $ %{} 'TestEntry (:name |callable-alias-known-map)
            :code $ quote $ let
                row $ {} (:finished-time 7) (:title |task)
              assert=
                group-open-rows $ {} $ |one row
                group-open-rows-alias $ {} $ |one row
            :tags $ #{} :callback-container-shape :unit
        'group-open-rows-arrow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn group-open-rows-arrow (rows)
            -> rows (&map:vals)
              group-by $ fn (row) (&map:get row :finished-time)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'String (:: 'Map 'Tag 'Dynamic)
            :return $ :: 'Map 'Dynamic $ :: 'List (:: 'Map 'Tag 'Dynamic)
          :tests $ [] $ %{} 'TestEntry (:name |threaded-known-map)
            :code $ quote $ let
                row $ {} (:finished-time 7) (:title |task)
              assert=
                group-open-rows $ {} $ |one row
                group-open-rows-arrow $ {} $ |one row
            :tags $ #{} :callback-container-shape :unit
        'read-open-members $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-open-members (rows)
            map rows $ fn (row) (&map:get row :title)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Dynamic)
            :return $ :: 'List 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |map-keeps-member-dynamic)
            :code $ quote $ assert= ([] |task 42)
              read-open-members $ []
                {} $ :title |task
                {} $ :title 42
            :tags $ #{} :callback-container-shape :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.callback-shape
    'app.dynamic-construction $ %{} 'FileEntry
      :defs $ {}
        'EmptyBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct EmptyBox
          :examples $ []
          :schema $ :: 'StructDef
        'PairBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct PairBox (:a 'Dynamic) (:b 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'checked-create $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-create (prototype key value)
            try
              %{} prototype $ key value
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |scalar)
              :code $ quote $ do
                assert= (Box :value 1) (checked-create Box :value 1)
                assert= nil $ checked-create Box :value |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |list)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1 2
                  checked-create ListBox :items $ [] 1 2
                assert= nil $ checked-create ListBox :items $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |map)
              :code $ quote $ do
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  checked-create MapBox :items $ {} $ :a ([] 1 nil)
                assert= nil $ checked-create MapBox :items $ {}
                  :a $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |set)
              :code $ quote $ do
                assert=
                  SetBox :items $ #{} 1 2
                  checked-create SetBox :items $ #{} 1 2
                assert= nil $ checked-create SetBox :items $ #{} |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |nominal)
              :code $ quote $ do
                assert=
                  NominalBox :user $ app.field-owner/User :name |one
                  checked-create NominalBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-create NominalBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |alias)
              :code $ quote $ do
                assert=
                  AliasBox :user $ app.field-owner/User :name |one
                  checked-create AliasBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-create AliasBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |applied)
              :code $ quote $ do
                assert=
                  AppliedBox :item $ GenericBox :items $ [] 1
                  checked-create AppliedBox :item $ GenericBox :items $ [] 1
                assert= nil $ checked-create AppliedBox :item $ GenericBox :items ([] |wrong)
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ do
                assert=
                  OptionBox :item $ Option :some 1
                  checked-create OptionBox :item $ Option :some 1
                assert= nil $ checked-create OptionBox :item $ Option :some |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |empty-and-open)
              :code $ quote $ do
                assert=
                  ListBox :items $ []
                  checked-create ListBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-create OpenBox :items $ [] 1 |two nil
                assert=
                  GenericBox :items $ [] 1
                  checked-create GenericBox :items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |field-name-boundary)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1
                  checked-create ListBox |items $ [] 1
                assert= nil $ checked-create ListBox :missing $ [] 1
                assert= nil $ checked-create ListBox 42 $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |symbol-field-name)
              :code $ quote $ assert=
                ListBox :items $ [] 1
                checked-create ListBox 'items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
        'checked-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-empty (prototype)
            try (%{} prototype)
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef
          :tests $ [] $ %{} 'TestEntry (:name |zero-field-and-missing)
            :code $ quote $ do
              assert=
                checked-map EmptyBox $ {}
                checked-empty EmptyBox
              assert= true $ struct? $ checked-empty EmptyBox
              assert= nil $ checked-empty PairBox
              assert= nil $ checked-map EmptyBox $ {} (:extra nil)
            :tags $ #{} :checked-struct-construction :unit
        'checked-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-map (prototype fields)
            try (&struct:from-map prototype fields)
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef $ :: 'Map 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |complete-and-immutable)
              :code $ quote $ let
                  fields $ {} (:b |two) (:a nil)
                assert= (PairBox :a nil :b |two) (checked-map PairBox fields)
                assert=
                  {} (:b |two) (:a nil)
                  , fields
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |missing-and-extra)
              :code $ quote $ do
                assert= nil $ checked-map PairBox $ {} (:a nil)
                assert= nil $ checked-map PairBox $ {} (:a nil) (:b nil) (:extra nil)
                assert= nil $ checked-map PairBox $ {} (:a nil) (:extra nil)
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |normalized-duplicate)
              :code $ quote $ do
                assert= nil $ checked-map PairBox $ {} (:a nil) (|a 2)
                assert= nil $ checked-map PairBox $ {} (|a nil) (|b 2) (:a 3)
              :tags $ #{} :checked-struct-construction :unit
        'checked-map-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-map-field (prototype key value)
            try
              &struct:from-map prototype $ {} $ key value
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |scalar)
              :code $ quote $ do
                assert= (Box :value 1) (checked-map-field Box :value 1)
                assert= nil $ checked-map-field Box :value |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |list)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1 2
                  checked-map-field ListBox :items $ [] 1 2
                assert= nil $ checked-map-field ListBox :items $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |map)
              :code $ quote $ do
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  checked-map-field MapBox :items $ {} $ :a ([] 1 nil)
                assert= nil $ checked-map-field MapBox :items $ {}
                  :a $ [] |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |set)
              :code $ quote $ do
                assert=
                  SetBox :items $ #{} 1 2
                  checked-map-field SetBox :items $ #{} 1 2
                assert= nil $ checked-map-field SetBox :items $ #{} |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |nominal)
              :code $ quote $ do
                assert=
                  NominalBox :user $ app.field-owner/User :name |one
                  checked-map-field NominalBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-map-field NominalBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |alias)
              :code $ quote $ do
                assert=
                  AliasBox :user $ app.field-owner/User :name |one
                  checked-map-field AliasBox :user $ app.field-owner/User :name |one
                assert= nil $ checked-map-field AliasBox :user $ app.field-consumer/User :name |other
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |applied)
              :code $ quote $ do
                assert=
                  AppliedBox :item $ GenericBox :items $ [] 1
                  checked-map-field AppliedBox :item $ GenericBox :items $ [] 1
                assert= nil $ checked-map-field AppliedBox :item $ GenericBox :items ([] |wrong)
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ do
                assert=
                  OptionBox :item $ Option :some 1
                  checked-map-field OptionBox :item $ Option :some 1
                assert= nil $ checked-map-field OptionBox :item $ Option :some |wrong
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |empty-and-open)
              :code $ quote $ do
                assert=
                  ListBox :items $ []
                  checked-map-field ListBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-map-field OpenBox :items $ [] 1 |two nil
                assert=
                  GenericBox :items $ [] 1
                  checked-map-field GenericBox :items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |field-name-boundary)
              :code $ quote $ do
                assert=
                  ListBox :items $ [] 1
                  checked-map-field ListBox |items $ [] 1
                assert= nil $ checked-map-field ListBox :missing $ [] 1
                assert= nil $ checked-map-field ListBox 42 $ [] 1
              :tags $ #{} :checked-struct-construction :unit
            %{} 'TestEntry (:name |symbol-map-key-rejected)
              :code $ quote $ assert= nil
                checked-map-field ListBox 'items $ [] 1
              :tags $ #{} :checked-struct-construction :unit
        'checked-pairs $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-pairs (prototype k1 v1 k2 v2)
            try
              %{} prototype (k1 v1) (k2 v2)
              fn (_error) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'StructDef 'Dynamic 'Dynamic 'Dynamic 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |complete-order-and-duplicate)
            :code $ quote $ do
              assert= (PairBox :a nil :b |two) (checked-pairs PairBox :b |two :a nil)
              assert= nil $ checked-pairs PairBox :a nil |a 2
              assert= nil $ checked-pairs PairBox :missing nil :b 2
            :tags $ #{} :checked-struct-construction :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.dynamic-construction
          :require $ app.dynamic-write :refer $ Box ListBox MapBox SetBox NominalBox AliasBox AppliedBox GenericBox OptionBox OpenBox
    'app.dynamic-write $ %{} 'FileEntry
      :defs $ {}
        'AliasBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct AliasBox (:user 'app.dynamic-write/UserAlias)
          :examples $ []
          :schema $ :: 'StructDef
        'AppliedBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct AppliedBox
            :item $ :: 'app.dynamic-write/GenericBox 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'Box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Box (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'FunctionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct FunctionBox
            :apply $ :: 'Fn $ {}
              :args $ [] 'Number
              :return 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'GenericBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct GenericBox ([] 'T)
            :items $ :: 'List 'T
          :examples $ []
          :schema $ :: 'StructDef
        'HostScalarBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct HostScalarBox
            :value $ :: 'JsNullish 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'ListBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ListBox
            :items $ :: 'List 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'MapBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct MapBox
            :items $ :: 'Map 'Tag $ :: 'List (:: 'Optional 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'NestedRefBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NestedRefBox (:item 'app.dynamic-write/RefBox)
          :examples $ []
          :schema $ :: 'StructDef
        'NominalBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NominalBox (:user 'app.field-owner/User)
          :examples $ []
          :schema $ :: 'StructDef
        'NullableFunctionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NullableFunctionBox
            :value $ :: 'Optional $ :: 'Fn
              {}
                :args $ [] 'Number
                :return 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'NullableRefBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NullableRefBox
            :value $ :: 'Optional $ :: 'Ref 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'NullishFunctionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NullishFunctionBox
            :value $ :: 'JsNullish $ :: 'Fn
              {}
                :args $ [] 'Number
                :return 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'OpenBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OpenBox
            :items $ :: 'List 'Dynamic
          :examples $ []
          :schema $ :: 'StructDef
        'OpenCallableBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OpenCallableBox (:apply 'Fn)
          :examples $ []
          :schema $ :: 'StructDef
        'OpenRefBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OpenRefBox
            :cell $ :: 'Ref 'Dynamic
          :examples $ []
          :schema $ :: 'StructDef
        'OptionBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptionBox
            :item $ :: 'Option 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'RefBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct RefBox
            :cell $ :: 'Ref 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'SetBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct SetBox
            :items $ :: 'Set 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'UserAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def UserAlias app.field-owner/User
          :examples $ []
          :schema $ :: 'StructDef
        'checked-assoc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-assoc (base key value)
            if (struct? base)
              try (&struct:assoc base key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-assoc base :value 2)
                assert= nil $ checked-assoc base :value |wrong
                assert= nil $ checked-assoc base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :struct-boundary :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-assoc base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :deep-struct-boundary :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-assoc base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :nominal-struct-boundary :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-assoc base |items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-assoc base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-assoc base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-assoc base :items $ {}
                assert= nil $ checked-assoc base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-assoc base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-assoc base :items $ #{} 2 3
                assert= nil $ checked-assoc base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-assoc base :user next)
                assert= nil $ checked-assoc base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-assoc base :item two)
                assert= nil $ checked-assoc base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-assoc base :item $ Option :some 2
                assert= nil $ checked-assoc base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-assoc base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |erased-generic-cannot-prove)
              :code $ quote $ let
                  base $ GenericBox :items $ [] 1
                assert= nil $ checked-assoc base :items $ [] |wrong
                assert= nil $ checked-assoc base :items $ [] 2
                assert=
                  GenericBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |callable-signature-cannot-prove)
              :code $ quote $ let
                  base $ FunctionBox :apply number-id
                assert= nil $ checked-assoc base :apply string-id
                assert= nil $ checked-assoc base :apply number-id
                assert= (FunctionBox :apply number-id) base
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |mutable-payload-is-not-evidence)
              :code $ quote $ let
                  base $ RefBox :cell $ ref 1
                  next $ ref 2
                assert= nil $ checked-assoc base :cell next
                assert= 1 $ deref $ :cell base
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |nested-mutable-payload-is-not-evidence)
              :code $ quote $ let
                  inner $ RefBox :cell $ ref 1
                  base $ NestedRefBox :item inner
                assert= nil $ checked-assoc base :item inner
                assert= base $ NestedRefBox :item inner
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |nullish-scalar-boundary)
              :code $ quote $ let
                  base $ HostScalarBox :value nil
                assert= nil $ checked-assoc base :value |wrong
                assert= base $ checked-assoc base :value nil
                assert= (HostScalarBox :value 2) (checked-assoc base :value 2)
                assert= (HostScalarBox :value nil) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-ref)
              :code $ quote $ let
                  base $ OpenRefBox :cell $ ref (open-value 1)
                  next $ ref $ open-value |next
                assert= (OpenRefBox :cell next) (checked-assoc base :cell next)
                assert= nil $ checked-assoc base :cell |not-ref
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-callable)
              :code $ quote $ let
                  base $ OpenCallableBox :apply number-id
                assert= (OpenCallableBox :apply string-id) (checked-assoc base :apply string-id)
                assert= nil $ checked-assoc base :apply |not-fn
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nullable-absence-NullableRefBox)
              :code $ quote $ let
                  payload $ ref 1
                  base $ NullableRefBox :value payload
                assert= (NullableRefBox :value nil) (checked-assoc base :value nil)
                assert= nil $ checked-assoc base :value payload
                assert= nil $ checked-assoc base :value |wrong
                assert= base $ NullableRefBox :value payload
              :tags $ #{} :checked-struct-write :nullable-write :unit
            %{} 'TestEntry (:name |nullable-absence-NullableFunctionBox)
              :code $ quote $ let
                  payload number-id
                  base $ NullableFunctionBox :value payload
                assert= (NullableFunctionBox :value nil) (checked-assoc base :value nil)
                assert= nil $ checked-assoc base :value payload
                assert= nil $ checked-assoc base :value |wrong
                assert= base $ NullableFunctionBox :value payload
              :tags $ #{} :checked-struct-write :nullable-write :unit
            %{} 'TestEntry (:name |nullable-absence-NullishFunctionBox)
              :code $ quote $ let
                  payload number-id
                  base $ NullishFunctionBox :value payload
                assert= (NullishFunctionBox :value nil) (checked-assoc base :value nil)
                assert= nil $ checked-assoc base :value payload
                assert= nil $ checked-assoc base :value |wrong
                assert= base $ NullishFunctionBox :value payload
              :tags $ #{} :checked-struct-write :nullable-write :unit
        'checked-assoc-at $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-assoc-at (base key value)
            if (struct? base)
              try (&struct:assoc-at base 0 key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-assoc-at base :value 2)
                assert= nil $ checked-assoc-at base :value |wrong
                assert= nil $ checked-assoc-at base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-assoc-at base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-assoc-at base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-assoc-at base :items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-assoc-at base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-assoc-at base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-assoc-at base :items $ {}
                assert= nil $ checked-assoc-at base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-assoc-at base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-assoc-at base :items $ #{} 2 3
                assert= nil $ checked-assoc-at base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-assoc-at base :user next)
                assert= nil $ checked-assoc-at base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-assoc-at base :item two)
                assert= nil $ checked-assoc-at base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-assoc-at base :item $ Option :some 2
                assert= nil $ checked-assoc-at base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-assoc-at base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
        'checked-with $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-with (base key value)
            if (struct? base)
              try (&struct:with base key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-with base :value 2)
                assert= nil $ checked-with base :value |wrong
                assert= nil $ checked-with base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :struct-boundary :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-with base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :deep-struct-boundary :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-with base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :nominal-struct-boundary :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-with base |items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-with base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-with base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-with base :items $ {}
                assert= nil $ checked-with base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-with base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-with base :items $ #{} 2 3
                assert= nil $ checked-with base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-with base :user next)
                assert= nil $ checked-with base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-with base :item two)
                assert= nil $ checked-with base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-with base :item $ Option :some 2
                assert= nil $ checked-with base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-with base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |erased-generic-cannot-prove)
              :code $ quote $ let
                  base $ GenericBox :items $ [] 1
                assert= nil $ checked-with base :items $ [] |wrong
                assert= nil $ checked-with base :items $ [] 2
                assert=
                  GenericBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |callable-signature-cannot-prove)
              :code $ quote $ let
                  base $ FunctionBox :apply number-id
                assert= nil $ checked-with base :apply string-id
                assert= nil $ checked-with base :apply number-id
                assert= (FunctionBox :apply number-id) base
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |mutable-payload-is-not-evidence)
              :code $ quote $ let
                  base $ RefBox :cell $ ref 1
                  next $ ref 2
                assert= nil $ checked-with base :cell next
                assert= 1 $ deref $ :cell base
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |nested-mutable-payload-is-not-evidence)
              :code $ quote $ let
                  inner $ RefBox :cell $ ref 1
                  base $ NestedRefBox :item inner
                assert= nil $ checked-with base :item inner
                assert= base $ NestedRefBox :item inner
              :tags $ #{} :checked-struct-write :fail-closed-write :unit
            %{} 'TestEntry (:name |nullish-scalar-boundary)
              :code $ quote $ let
                  base $ HostScalarBox :value nil
                assert= nil $ checked-with base :value |wrong
                assert= base $ checked-with base :value nil
                assert= (HostScalarBox :value 2) (checked-with base :value 2)
                assert= (HostScalarBox :value nil) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-ref)
              :code $ quote $ let
                  base $ OpenRefBox :cell $ ref (open-value 1)
                  next $ ref $ open-value |next
                assert= (OpenRefBox :cell next) (checked-with base :cell next)
                assert= nil $ checked-with base :cell |not-ref
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-callable)
              :code $ quote $ let
                  base $ OpenCallableBox :apply number-id
                assert= (OpenCallableBox :apply string-id) (checked-with base :apply string-id)
                assert= nil $ checked-with base :apply |not-fn
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nullable-absence-NullableRefBox)
              :code $ quote $ let
                  payload $ ref 1
                  base $ NullableRefBox :value payload
                assert= (NullableRefBox :value nil) (checked-with base :value nil)
                assert= nil $ checked-with base :value payload
                assert= nil $ checked-with base :value |wrong
                assert= base $ NullableRefBox :value payload
              :tags $ #{} :checked-struct-write :nullable-write :unit
            %{} 'TestEntry (:name |nullable-absence-NullableFunctionBox)
              :code $ quote $ let
                  payload number-id
                  base $ NullableFunctionBox :value payload
                assert= (NullableFunctionBox :value nil) (checked-with base :value nil)
                assert= nil $ checked-with base :value payload
                assert= nil $ checked-with base :value |wrong
                assert= base $ NullableFunctionBox :value payload
              :tags $ #{} :checked-struct-write :nullable-write :unit
            %{} 'TestEntry (:name |nullable-absence-NullishFunctionBox)
              :code $ quote $ let
                  payload number-id
                  base $ NullishFunctionBox :value payload
                assert= (NullishFunctionBox :value nil) (checked-with base :value nil)
                assert= nil $ checked-with base :value payload
                assert= nil $ checked-with base :value |wrong
                assert= base $ NullishFunctionBox :value payload
              :tags $ #{} :checked-struct-write :nullable-write :unit
        'checked-with-at $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-with-at (base key value)
            if (struct? base)
              try (&struct:with-at base 0 key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |runtime-field-check)
              :code $ quote $ let
                  base $ Box :value 1
                assert= (Box :value 2) (checked-with-at base :value 2)
                assert= nil $ checked-with-at base :value |wrong
                assert= nil $ checked-with-at base :missing 2
                assert= (Box :value 1) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-wrong-list-element)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert= nil $ checked-with-at base :items $ [] |wrong
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |reject-foreign-nominal)
              :code $ quote $ let
                  user $ app.field-owner/User :name |original
                  foreign $ app.field-consumer/User :name |foreign
                  base $ NominalBox :user user
                assert= nil $ checked-with-at base :user foreign
                assert= (NominalBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |valid-list)
              :code $ quote $ let
                  base $ ListBox :items $ [] 1
                assert=
                  ListBox :items $ [] 2 3
                  checked-with-at base :items $ [] 2 3
                assert=
                  ListBox :items $ []
                  checked-with-at base :items $ []
                assert=
                  ListBox :items $ [] 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nested-map)
              :code $ quote $ let
                  base $ MapBox :items $ {}
                    :a $ [] 1 nil
                assert=
                  MapBox :items $ {} $ :b ([] 2 nil)
                  checked-with-at base :items $ {} $ :b ([] 2 nil)
                assert=
                  MapBox :items $ {}
                  checked-with-at base :items $ {}
                assert= nil $ checked-with-at base :items $ {}
                  |a $ [] 1
                assert= nil $ checked-with-at base :items $ {}
                  :a $ [] |wrong
                assert=
                  MapBox :items $ {} $ :a ([] 1 nil)
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |set-members)
              :code $ quote $ let
                  base $ SetBox :items $ #{} 1
                assert=
                  SetBox :items $ #{} 2 3
                  checked-with-at base :items $ #{} 2 3
                assert= nil $ checked-with-at base :items $ #{} |wrong
                assert=
                  SetBox :items $ #{} 1
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |nominal-alias)
              :code $ quote $ let
                  user $ app.field-owner/User :name |one
                  next $ app.field-owner/User :name |two
                  base $ AliasBox :user user
                assert= (AliasBox :user next) (checked-with-at base :user next)
                assert= nil $ checked-with-at base :user $ app.field-consumer/User :name |wrong
                assert= (AliasBox :user user) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |applied-nominal)
              :code $ quote $ let
                  one $ GenericBox :items $ [] 1
                  two $ GenericBox :items $ [] 2
                  base $ AppliedBox :item one
                assert= (AppliedBox :item two) (checked-with-at base :item two)
                assert= nil $ checked-with-at base :item $ GenericBox :items ([] |wrong)
                assert= (AppliedBox :item one) base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |enum-payload)
              :code $ quote $ let
                  base $ OptionBox :item $ Option :none
                assert=
                  OptionBox :item $ Option :some 2
                  checked-with-at base :item $ Option :some 2
                assert= nil $ checked-with-at base :item $ Option :some |wrong
                assert=
                  OptionBox :item $ Option :none
                  , base
              :tags $ #{} :checked-struct-write :unit
            %{} 'TestEntry (:name |explicit-open-leaf)
              :code $ quote $ let
                  base $ OpenBox :items $ []
                assert=
                  OpenBox :items $ [] 1 |two nil
                  checked-with-at base :items $ [] 1 |two nil
              :tags $ #{} :checked-struct-write :unit
        'number-id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn number-id (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'open-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-value (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'string-id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn string-id (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'write-function $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-function (box f) (.assoc box :apply f)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.dynamic-write/FunctionBox)
            :args $ [] 'app.dynamic-write/FunctionBox $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |static-callable-evidence)
            :code $ quote $ let
                base $ FunctionBox :apply number-id
                updated $ write-function base number-id
              assert= 1 $
                :apply updated
                , 1
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-function-core $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-function-core (box value) (assoc box :apply value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.dynamic-write/FunctionBox)
            :args $ [] 'app.dynamic-write/FunctionBox $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |core-assoc-preserves-static-evidence)
            :code $ quote $ let
                base $ FunctionBox :apply number-id
                updated $ write-function-core base number-id
              assert= 1 $
                :apply updated
                , 1
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-function-core-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-function-core-string (box value) (assoc box |apply value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.dynamic-write/FunctionBox)
            :args $ [] 'app.dynamic-write/FunctionBox $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |core-assoc-preserves-static-evidence)
            :code $ quote $ let
                base $ FunctionBox :apply number-id
                updated $ write-function-core-string base number-id
              assert= 1 $
                :apply updated
                , 1
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-function-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-function-string (box f) (&struct:assoc box |apply f)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.dynamic-write/FunctionBox)
            :args $ [] 'app.dynamic-write/FunctionBox $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |static-string-callable)
            :code $ quote $ let
                base $ FunctionBox :apply number-id
                updated $ write-function-string base number-id
              assert= 1 $
                :apply updated
                , 1
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-function-with $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-function-with (box f) (&struct:with box |apply f)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.dynamic-write/FunctionBox)
            :args $ [] 'app.dynamic-write/FunctionBox $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |static-string-callable)
            :code $ quote $ let
                base $ FunctionBox :apply number-id
                updated $ write-function-with base number-id
              assert= 1 $
                :apply updated
                , 1
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-generic (box values) (.assoc box :items values)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'app.dynamic-write/GenericBox 'T) (:: 'List 'T)
            :generics $ [] 'T
            :return $ :: 'app.dynamic-write/GenericBox 'T
          :tests $ [] $ %{} 'TestEntry (:name |static-method-evidence)
            :code $ quote $ let
                base $ GenericBox :items $ [] 1
              assert=
                GenericBox :items $ [] 2
                write-generic base $ [] 2
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-generic-core $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-generic-core (box value) (assoc box :items value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'app.dynamic-write/GenericBox 'T) (:: 'List 'T)
            :generics $ [] 'T
            :return $ :: 'app.dynamic-write/GenericBox 'T
          :tests $ [] $ %{} 'TestEntry (:name |core-assoc-preserves-static-evidence)
            :code $ quote $ let
                base $ GenericBox :items $ [] 1
              assert=
                GenericBox :items $ [] 2
                write-generic-core base $ [] 2
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-generic-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-generic-string (box values) (&struct:assoc box |items values)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'app.dynamic-write/GenericBox 'T) (:: 'List 'T)
            :generics $ [] 'T
            :return $ :: 'app.dynamic-write/GenericBox 'T
          :tests $ [] $ %{} 'TestEntry (:name |static-string-generic)
            :code $ quote $ let
                base $ GenericBox :items $ [] 1
              assert=
                GenericBox :items $ [] 2
                write-generic-string base $ [] 2
            :tags $ #{} :checked-struct-write :static-write :unit
        'write-ref $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-ref (box value) (.assoc box :cell value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.dynamic-write/RefBox)
            :args $ [] 'app.dynamic-write/RefBox $ :: 'Ref 'Number
          :tests $ [] $ %{} 'TestEntry (:name |static-ref-evidence)
            :code $ quote $ let
                base $ RefBox :cell $ ref 1
                next $ ref 2
              assert= 2 $ deref $ :cell (write-ref base next)
            :tags $ #{} :checked-struct-write :static-write :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.dynamic-write
    'app.empty-evidence $ %{} 'FileEntry
      :defs $ {}
        'accumulate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn accumulate (limit)
            loop
                items $ {}
                n limit
              if (= n 0) items $ recur (.assoc items n n) (- n 1)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Number
            :return $ :: 'Map 'Number 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-loop-gains-members)
            :code $ quote $ do
              assert= ({}) (accumulate 0)
              assert=
                {} (1 1) (2 2)
                accumulate 2
            :tags $ #{} :empty-collection-evidence
        'accumulate-receiver-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn accumulate-receiver-first (limit)
            loop
                items $ {}
                n limit
              if (= n 0) items $ recur (items .assoc n n) (- n 1)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Number
            :return $ :: 'Map 'Number 'Number
          :tests $ [] $ %{} 'TestEntry (:name |receiver-first-empty-loop-gains-members)
            :code $ quote $ do
              assert= ({}) (accumulate-receiver-first 0)
              assert=
                {} (1 1) (2 2)
                accumulate-receiver-first 2
            :tags $ #{} :empty-collection-evidence
        'aliased-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn aliased-assignment (value)
            let
                item value
              if (number? item)
                do
                  assert= true $ number? $ &+ item 1
                  set! item |changed
              , item
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |local-alias-write-contract)
            :code $ quote $ do
              assert= |changed $ aliased-assignment 1
              assert= |original $ aliased-assignment |original
            :tags $ #{} :empty-collection-assignment
        'dead-branch-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn dead-branch-assignment (value)
            if (number? value)
              do
                if true &unit $ set! value |changed
                &+ value 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |dead-write-keeps-live-evidence)
            :code $ quote $ assert= 2 (dead-branch-assignment 1)
            :tags $ #{} :empty-collection-assignment
        'empty-lookup $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn empty-lookup ()
            let
                items $ {}
              get items :missing
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Option 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-map-accepts-lookup-key)
            :code $ quote $ assert= (Option :none) (empty-lookup)
            :tags $ #{} :empty-collection-evidence
        'fold-counts $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn fold-counts (words)
            let
                init $ {}
              foldl words init $ fn (acc w)
                let
                    n $ -> (get acc w) (.unwrap-or 0)
                  assoc acc w $ inc n
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Tag
            :return $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |infers-empty-named-assoc-reducer)
            :code $ quote $ do
              assert=
                {} (:a 3) (:b 2) (:c 1)
                fold-counts $ [] :a :b :a :c :a :b
              assert= ({})
                fold-counts $ []
            :tags $ #{} :empty-collection-evidence
        'grow-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn grow-list ()
            let
                items $ []
              &list:append items 1
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-list-gains-member)
            :code $ quote $ assert= ([] 1) (grow-list)
            :tags $ #{} :empty-collection-evidence
        'grow-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn grow-map ()
            let
                items $ {}
              .assoc items |a 1
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Map 'String 'Number
          :tests $ [] $ %{} 'TestEntry (:name |immutable-map-growth)
            :code $ quote $ assert=
              {} $ |a 1
              grow-map
            :tags $ #{} :empty-collection-evidence
        'grow-set $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn grow-set ()
            let
                items $ #{}
              .include items 1
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Set 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-set-gains-member)
            :code $ quote $ assert= (#{} 1) (grow-set)
            :tags $ #{} :empty-collection-evidence
        'incompatible-generics $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn incompatible-generics (xs ys flag)
            let
                result $ if flag xs ys
              assert-type result $ :: 'List 'T
              , result
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'List 'T) (:: 'List 'U) 'Bool
            :generics $ [] 'T 'U
            :return $ :: 'List 'T
        'list-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn list-alias ()
            let
                items $ []
              assert-type items $ :: 'List 'Number
              , items
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-list-alias)
            :code $ quote $ assert= ([]) (list-alias)
            :tags $ #{} :empty-collection-evidence
        'map-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-alias ()
            let
                items $ {}
              assert-type items $ :: 'Map 'String 'Number
              , items
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Map 'String 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-map-alias)
            :code $ quote $ assert= ({}) (map-alias)
            :tags $ #{} :empty-collection-evidence
        'match-peer-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn match-peer-assignment (value choice)
            if (number? value)
              match choice
                (:some item)
                  do (set! value |changed) value
                _ $ &+ value 1
              , value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'Option 'Number
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |match-branches-use-independent-evidence)
            :code $ quote $ do
              assert= |changed $ match-peer-assignment 1 $ Option :some 2
              assert= 2 $ match-peer-assignment 1 $ Option :none
            :tags $ #{} :empty-collection-assignment
        'narrowed-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn narrowed-assignment (value)
            if (number? value) (set! value |changed)
            , value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |dynamic-binding-keeps-write-contract)
            :code $ quote $ do
              assert= |changed $ narrowed-assignment 1
              assert= |original $ narrowed-assignment |original
            :tags $ #{} :empty-collection-assignment
        'nested-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested-empty ()
            let
                rows $ [] $ {}
              assert-type rows $ :: 'List $ :: 'Map 'String 'Number
              , rows
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'List $ :: 'Map 'String 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nested-empty-map)
            :code $ quote $ assert=
              [] $ {}
              nested-empty
            :tags $ #{} :empty-collection-evidence
        'open-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-map (items)
            assert-type items $ :: 'Map 'String 'Number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'Dynamic 'Dynamic
            :return $ :: 'Map 'String 'Number
        'rebound-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rebound-map ()
            let
                items $ {}
              set! items $ {} $ |a |wrong
              assert-type items $ :: 'Map 'String 'Number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :features $ #{} :js-ffi
            :return $ :: 'Map 'String 'Number
        'ref-invariant $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn ref-invariant ()
            let
                cell $ ref $ assert-type ({}) (:: 'Map 'String 'Number)
              assert-type cell $ :: 'Ref $ :: 'Map 'String 'Dynamic
          :examples $ []
        'result-seed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn result-seed ()
            let
                seed $ Result :ok $ {}
              assert-type seed $ :: 'Result (:: 'Map 'String 'Number) 'String
              , seed
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Result (:: 'Map 'String 'Number) 'String
          :tests $ [] $ %{} 'TestEntry (:name |empty-result-payload)
            :code $ quote $ assert=
              Result :ok $ {}
              result-seed
            :tags $ #{} :empty-collection-evidence
        'set-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn set-alias ()
            let
                items $ #{}
              assert-type items $ :: 'Set 'Number
              , items
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Set 'Number
          :tests $ [] $ %{} 'TestEntry (:name |empty-set-alias)
            :code $ quote $ assert= (#{}) (set-alias)
            :tags $ #{} :empty-collection-evidence
        'shadow-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn shadow-map (raw)
            let
                items $ {}
              let
                  items raw
                assert-type items $ :: 'Map 'String 'Number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'Dynamic 'Dynamic
            :return $ :: 'Map 'String 'Number
        'shadowed-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn shadowed-assignment (value)
            if (number? value)
              do
                let
                    value 1
                  set! value 2
                  assert= 2 value
                assert= 2 $ &+ value 1
                set! value |changed
            , value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |shadowing-restores-outer-contract)
            :code $ quote $ assert= |changed (shadowed-assignment 1)
            :tags $ #{} :empty-collection-assignment
        'shadowed-match-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn shadowed-match-assignment (value choice)
            if (number? value)
              do
                match choice
                  (:some value) (set! value 2)
                  (:none) &unit
                &+ value 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic $ :: 'Option 'Number
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |payload-binding-does-not-write-outer-local)
            :code $ quote $ do
              assert= 2 $ shadowed-match-assignment 1 $ Option :some 8
              assert= 2 $ shadowed-match-assignment 1 $ Option :none
            :tags $ #{} :empty-collection-assignment
        'stale-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn stale-assignment (value)
            if (number? value)
              do (set! value |changed) (&+ value 1)
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
        'stale-branch-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn stale-branch-assignment (value flag)
            if (number? value)
              do
                if flag $ set! value |changed
                &+ value 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic 'Bool
            :features $ #{} :js-ffi
        'stale-let-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn stale-let-assignment (value)
            if (number? value)
              do
                let
                    ignored 0
                  set! value |changed
                &+ value 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
        'stale-match-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn stale-match-assignment (value choice)
            if (number? value)
              do
                match choice
                  (:some item) (set! value |changed)
                  (:none) &unit
                &+ value 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic $ :: 'Option 'Number
            :features $ #{} :js-ffi
        'typed-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn typed-assignment ()
            let
                items $ assert-type ({}) (:: 'Map 'String 'Number)
              set! items $ {} $ |a 1
              , items
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :features $ #{} :js-ffi
            :return $ :: 'Map 'String 'Number
          :tests $ [] $ %{} 'TestEntry (:name |assignment-preserves-explicit-contract)
            :code $ quote $ assert=
              {} $ |a 1
              typed-assignment
            :tags $ #{} :empty-collection-assignment
        'typed-ref $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn typed-ref ()
            let
                slot $ ref $ assert-type ({}) (:: 'Map 'String 'Number)
              reset! slot $ {} $ |a 1
              deref slot
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Map 'String 'Number
          :tests $ [] $ %{} 'TestEntry (:name |explicit-ref-initializer-contract)
            :code $ quote $ assert=
              {} $ |a 1
              typed-ref
            :tags $ #{} :empty-collection-ref
        'wrong-grown-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-grown-list ()
            let
                items $ []
              assert-type (.append items |wrong) (:: 'List 'Number)
          :examples $ []
        'wrong-grown-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-grown-map ()
            let
                items $ {}
              assert-type (&map:assoc items |a |wrong) (:: 'Map 'String 'Number)
          :examples $ []
        'wrong-grown-set $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-grown-set ()
            let
                items $ #{}
              assert-type (.include items |wrong) (:: 'Set 'Number)
          :examples $ []
        'wrong-map-member $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-map-member ()
            let
                items $ {} $ |a |wrong
              assert-type items $ :: 'Map 'String 'Number
          :examples $ []
        'wrong-match-payload-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-match-payload-assignment (value choice)
            match choice
              (:some value) (set! value |wrong)
              (:none) &unit
            , value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'Option 'Number
            :features $ #{} :js-ffi
        'wrong-ref-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-ref-alias ()
            let
                cell $ ref $ assert-type ({}) (:: 'Map 'String 'Number)
                alias cell
              reset! alias $ {} $ |a |wrong
          :examples $ []
        'wrong-ref-write $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-ref-write ()
            let
                cell $ ref $ {}
              reset! cell $ {} $ |a 1
          :examples $ []
        'wrong-typed-assignment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrong-typed-assignment (value)
            if (number? value) (set! value |wrong)
            , value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
            :features $ #{} :js-ffi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.empty-evidence
    'app.empty-fields $ %{} 'FileEntry
      :defs $ {}
        'Fields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Fields
            :mapping $ :: 'Optional $ :: 'Map 'Tag 'String
            :items $ :: 'Optional $ :: 'List 'Number
            :members $ :: 'Optional $ :: 'Set 'String
          :examples $ []
          :schema $ :: 'StructDef
        'open-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-map () ({})
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'Map 'Tag 'Dynamic
        'verify-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-empty ()
            let
                value $ Fields :mapping ({}) :items ([]) :members $ #{}
              assert= ({}) (:mapping value)
              assert= ([]) (:items value)
              assert= (#{}) (:members value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |optional-empty-container-literals)
            :code $ quote $ verify-empty
            :tags $ #{} :optional-empty-field
        'verify-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-nil ()
            let
                value $ Fields :mapping nil :items nil :members nil
              assert= nil $ :mapping value
              assert= nil $ :items value
              assert= nil $ :members value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |optional-nil-container-fields)
            :code $ quote $ verify-nil
            :tags $ #{} :optional-empty-field
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.empty-fields
    'app.field-consumer $ %{} 'FileEntry
      :defs $ {}
        'User $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct User (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'make-local-user $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-local-user () (User :name |Caller)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.field-consumer/User)
            :args $ []
        'verify-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-generic ()
            let
                user $ owner/make-user
                value $ owner/Envelope :user user :value 42
              assert-type (:value value) 'Number
              assert= 42 $ :value value
              assert= |Ada $ :name $ :user value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-generic)
            :code $ quote $ verify-generic
            :tags $ #{} :struct-field-origin
        'verify-generic-origin $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-generic-origin ()
            let
                value $ owner/Envelope :user (owner/make-user) :value $ make-local-user
              assert-type (:value value) 'app.field-consumer/User
              assert= |Caller $ :name $ :value value
              assert= |Ada $ :name $ :user value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |caller-owned-generic-argument)
            :code $ quote $ verify-generic-origin
            :tags $ #{} :struct-field-origin
        'verify-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-map ()
            let
                user $ owner/make-user
                db $ owner/Database :users
                  {} $ |one user
                  , :maybe nil
                found $ -> (:users db) (.get |one) (.unwrap)
              assert= |Ada $ :name found
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-map)
            :code $ quote $ verify-map
            :tags $ #{} :struct-field-origin
        'verify-optional $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-optional ()
            let
                user $ owner/make-user
                db $ owner/Database :users
                  {} $ |one user
                  , :maybe user
              assert= user $ :maybe db
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-optional)
            :code $ quote $ verify-optional
            :tags $ #{} :struct-field-origin
        'verify-update $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-update ()
            let
                user $ owner/make-user
                original $ owner/Database :users
                  {} $ |one user
                  , :maybe nil
                updated $ original .assoc :maybe user
                changed $ updated .assoc :users $ {} (|two user)
              assert= nil $ :maybe original
              assert= user $ :maybe changed
              assert= (Option :none)
                get (:users original) |two
              assert= (Option :some user)
                get (:users changed) |two
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |declaration-owned-update)
            :code $ quote $ verify-update
            :tags $ #{} :struct-field-origin
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.field-consumer
          :require $ app.field-owner :as owner
    'app.field-owner $ %{} 'FileEntry
      :defs $ {}
        'Database $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Database
            :users $ :: 'Map 'String 'User
            :maybe $ :: 'Optional 'User
          :examples $ []
          :schema $ :: 'StructDef
        'Envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Envelope ([] 'T) (:user 'User) (:value 'T)
          :examples $ []
          :schema $ :: 'StructDef
        'User $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct User (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'make-user $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-user () (User :name |Ada)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.field-owner/User)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.field-owner
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'answer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def answer 42
          :examples $ []
          :schema $ :: 'Number
        'initial-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def initial-state values/initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
        'label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def label |Ada
          :examples $ []
          :schema $ :: 'String
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (verify-values) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'members $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def members
            {} $ 1 |Ada
          :examples $ []
          :schema $ :: 'Map 'Number 'String
        'open-store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def open-store members
          :examples $ []
          :schema $ :: 'Dynamic
        'prepare-client-patch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn prepare-client-patch (next-raw)
            match (try-decode-map-as next-raw app.values/ClientProjection)
              (:ok typed)
                Result :ok $ app.values/PatchCandidate :raw next-raw :typed typed
              (:err reason) (Result :err reason)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :return $ :: 'Result 'app.values/PatchCandidate 'String
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'state-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
        'verify-login-decode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-login-decode ()
            match
              try-decode-map-as
                {} (:username |Ada) (:password |secret)
                , app.values/LoginState
              (:ok state)
                assert= |Ada $ :username state
              (:err reason) (raise reason)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |checked-login-projection)
              :code $ quote $ do (verify-login-decode)
                match
                  try-decode-map-as
                    {} (:username |Ada) (:password 42)
                    , app.values/LoginState
                  (:ok _) (raise |invalid-password-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |password
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |persisted-nested-value)
              :code $ quote $ do
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ {} $ :years ([] 2025 2026)
                    :: 'Map 'Tag $ :: 'List 'Number
                  (:ok data)
                    assert=
                      Option :some $ [] 2025 2026
                      get data :years
                  (:err reason) (raise reason)
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ {} $ :years ([] 2025 |bad)
                    :: 'Map 'Tag $ :: 'List 'Number
                  (:ok _) (raise |invalid-storage-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |[1]
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |member-key-contract)
              :code $ quote $ do
                match
                  try-decode-map-as members $ :: 'Map 'Number 'String
                  (:ok data)
                    assert= (Option :some |Ada) (get data 1)
                  (:err reason) (raise reason)
                match
                  try-decode-map-as members $ :: 'Map 'String 'String
                  (:ok _) (raise |numeric-member-key-accepted-as-string)
                  (:err reason)
                    assert= true $ .includes? reason |key
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |restored-credentials)
              :code $ quote $ do
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ [] |Ada |secret
                    :: 'List 'String
                  (:ok credentials)
                    assert= ([] |Ada |secret) credentials
                  (:err reason) (raise reason)
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ [] |Ada 42
                    :: 'List 'String
                  (:ok _) (raise |invalid-credentials-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |[1]
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |nominal-component-state)
              :code $ quote $ do (verify-values)
                assert= |Ada $ :username $ assoc initial-state :username |Ada
                assert= | $ :username initial-state
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |checked-patch-projection)
              :code $ quote $ let
                  previous-raw $ {} (:count 1) (:color |red)
                  previous $ match (prepare-client-patch previous-raw)
                    (:ok candidate) candidate
                    (:err reason) (raise reason)
                  next-raw $ assoc previous-raw :count 2
                match (prepare-client-patch next-raw)
                  (:ok candidate)
                    do
                      assert= next-raw $ :raw candidate
                      assert= 2 $ :count $ :typed candidate
                      assert= |red $ :color $ :typed candidate
                      assert= previous-raw $ :raw previous
                      assert= 1 $ :count $ :typed previous
                  (:err reason) (raise reason)
                match
                  prepare-client-patch $ assoc next-raw :count |invalid
                  (:ok _) (raise |invalid-patch-candidate-accepted)
                  (:err reason)
                    do
                      assert= true $ .includes? reason |count
                      assert= previous-raw $ :raw previous
                      assert= 1 $ :count $ :typed previous
                match
                  prepare-client-patch $ {} $ :count 2
                  (:ok _) (raise |incomplete-patch-candidate-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |color
              :tags $ #{} :diary-boundary :unit
        'verify-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-values ()
            assert= | $ :username initial-state
            assert= | $ :username state-alias
            assert= | $ :password reader/state-alias
            assert= 1 $ .len members
            assert= 42 answer
            assert= |Ada label
            assert= members open-store
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |retains-typed-top-level-values)
              :code $ quote $ verify-values
              :tags $ #{} :def-value-contract :unit
            %{} 'TestEntry (:name |optional-builtin-unary-callback)
              :code $ quote $ assert= ([] "|do 1" "|do 2")
                map
                  map ([] 1 2) format-cirru-edn
                  , trim
              :tags $ #{} :optional-callable :unit
            %{} 'TestEntry (:name |optional-builtin-callback-alias)
              :code $ quote $ let
                  trim-text trim
                  formatted format-cirru-edn
                assert= ([] |first |second)
                  map ([] "| first " "| second ") trim-text
                assert= "|do 42" $ trim-text $ formatted 42 false
              :tags $ #{} :optional-callable :unit
            %{} 'TestEntry (:name |nominal-option-callback-omission)
              :code $ quote $ let
                  with-extra $ fn (value extra)
                    hint-fn $ {}
                      :args $ [] 'Number $ :: 'Option 'Number
                      :return 'Number
                    + value $ extra .unwrap-or 10
                assert= ([] 11 12)
                  map ([] 1 2) with-extra
                assert= 4 $ with-extra 1 $ Option :some 3
              :tags $ #{} :optional-callable :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require (app.reader :as reader) (app.values :as values)
    'app.reader $ %{} 'FileEntry
      :defs $ {} $ 'state-alias
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias values/initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.reader
          :require $ app.values :as values
    'app.short-circuit $ %{} 'FileEntry
      :defs $ {}
        'common-or $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn common-or (k flag)
            if
              or
                and (number? k) flag
                and (number? k) (&>= k 0)
              &+ k 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |both-or-paths)
            :code $ quote $ do
              assert= 2 $ common-or 1 false
              assert= -1 $ common-or -2 true
              assert= 0 $ common-or |bad true
            :tags $ #{} :short-circuit-proof :unit
        'compound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compound (k)
            if
              and (number? k)
                = k $ floor k
                &>= k 0
                &< k $ count $ [] 1 2
              &list:assoc ([] 1 2) k 3
              [] 1 2
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :return $ :: 'List 'Number
          :tests $ []
            %{} 'TestEntry (:name |checked-index)
              :code $ quote $ do
                assert= ([] 1 3) (compound 1)
                assert= ([] 1 2) (compound |bad)
                assert= ([] 1 2) (compound -1)
                assert= ([] 1 2) (compound 1.5)
                assert= ([] 1 2) (compound 2)
              :tags $ #{} :short-circuit-proof :unit
            %{} 'TestEntry (:name |single-evaluation)
              :code $ quote $ let
                  calls $ atom 0
                assert= false $ and false $ do
                  reset! calls $ inc $ deref calls
                  , true
                assert= 0 $ deref calls
                assert= 7 $ and true $ do
                  reset! calls $ inc $ deref calls
                  , 7
                assert= 1 $ deref calls
                assert= 7 $ or 7 $ do (reset! calls 99) false
                assert= 1 $ deref calls
              :tags $ #{} :short-circuit-proof :unit
        'false-path $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn false-path (k)
            if
              if (number? k) false true
              , 0 $ &+ k 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |false-branch-proof)
            :code $ quote $ do
              assert= 2 $ false-path 1
              assert= 0 $ false-path |bad
            :tags $ #{} :short-circuit-proof :unit
        'lexical-guard $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-guard (k)
            if
              let
                  valid $ number? k
                if valid true false
              &+ k 1
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |bound-predicate)
            :code $ quote $ do
              assert= 2 $ lexical-guard 1
              assert= 0 $ lexical-guard |bad
            :tags $ #{} :short-circuit-proof :unit
        'nested $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nested (k)
            if (number? k)
              if
                and
                  = k $ floor k
                  &>= k 0
                  &< k $ count $ [] 1 2
                &list:assoc ([] 1 2) k 3
                [] 1 2
              [] 1 2
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :return $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |checked-index)
            :code $ quote $ do
              assert= ([] 1 3) (nested 1)
              assert= ([] 1 2) (nested |bad)
              assert= ([] 1 2) (nested -1)
              assert= ([] 1 2) (nested 1.5)
              assert= ([] 1 2) (nested 2)
            :tags $ #{} :short-circuit-proof :unit
        'two-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn two-values (x y)
            if
              and (number? x) (number? y) (&< x y)
              &+ x y
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |both-operands)
            :code $ quote $ do
              assert= 5 $ two-values 2 3
              assert= 0 $ two-values 2 |bad
              assert= 0 $ two-values |bad 3
            :tags $ #{} :short-circuit-proof :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.short-circuit
    'app.sort-selector $ %{} 'FileEntry
      :defs $ {}
        'alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias (rows)
            let
                order &list:sort-by
              order rows $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              alias $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-context :unit
        'alias-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias-chain (rows)
            let
                order &list:sort-by
                copy-order order
              copy-order rows $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              alias-chain $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-context :unit
        'checked-open-selector $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-open-selector (rows)
            &list:sort-by rows $ fn (row)
              hint-fn $ {}
                :args $ [] 'Dynamic
                :return 'Number
              decode-map-as row 'Number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Dynamic
            :return $ :: 'List 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |decodes-sort-key)
            :code $ quote $ assert= ([] 1 2)
              checked-open-selector $ [] 2 1
            :tags $ #{} :sort-selector-context :unit
        'copied-alias-survives-write $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn copied-alias-survives-write (rows)
            let
                order &list:sort-by
                copy-order order
              set! order &list:sort-by
              copy-order rows $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :features $ #{} :js-ffi
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              copied-alias-survives-write $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-assignment :unit
        'direct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn direct (rows)
            &list:sort-by rows $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ []
            %{} 'TestEntry (:name |orders-typed-rows)
              :code $ quote $ assert=
                []
                  {} $ :n 1
                  {} $ :n 2
                direct $ []
                  {} $ :n 2
                  {} $ :n 1
              :tags $ #{} :sort-selector-context :unit
            %{} 'TestEntry (:name |retains-tag-selector)
              :code $ quote $ assert=
                []
                  {} $ :n 1
                  {} $ :n 2
                &list:sort-by
                  []
                    {} $ :n 2
                    {} $ :n 1
                  , :n
              :tags $ #{} :sort-selector-context :unit
        'ignore-selector $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn ignore-selector (rows selector) rows
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'List 'T) 'Dynamic
            :generics $ [] 'T
            :return $ :: 'List 'T
        'method $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn method (rows)
            rows .sort-by $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ []
            %{} 'TestEntry (:name |orders-typed-rows)
              :code $ quote $ assert=
                []
                  {} $ :n 1
                  {} $ :n 2
                method $ []
                  {} $ :n 2
                  {} $ :n 1
              :tags $ #{} :sort-selector-context :unit
            %{} 'TestEntry (:name |retains-tag-selector)
              :code $ quote $ let
                  rows $ []
                    {} $ :n 2
                    {} $ :n 1
                  key :n
                assert=
                  []
                    {} $ :n 1
                    {} $ :n 2
                  rows .sort-by key
                assert=
                  []
                    {} $ :n 1
                    {} $ :n 2
                  .sort-by rows key
              :tags $ #{} :sort-selector-context :unit
        'mutable-tag-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mutable-tag-alias (rows)
            let
                order &list:sort-by
              order rows :n
              set! order &list:sort-by
              order rows :n
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :features $ #{} :js-ffi
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              mutable-tag-alias $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-assignment :unit
        'named-selector $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn named-selector (rows)
            &list:sort-by rows $ defn select-key (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              named-selector $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-context :unit
        'prefix-method $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn prefix-method (rows)
            .sort-by rows $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              prefix-method $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-context :unit
        'restored-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn restored-alias (rows)
            let
                order &list:sort-by
              let
                  order :inner
                assert= :inner order
              order rows $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              restored-alias $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-context :unit
        'threaded $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn threaded (rows)
            -> rows $ &list:sort-by $ fn (row) (&map:get row :n)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'Map 'Tag 'Number)
            :return $ :: 'List $ :: 'Map 'Tag 'Number
          :tests $ [] $ %{} 'TestEntry (:name |orders-typed-rows)
            :code $ quote $ assert=
              []
                {} $ :n 1
                {} $ :n 2
              threaded $ []
                {} $ :n 2
                {} $ :n 1
            :tags $ #{} :sort-selector-context :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.sort-selector
    'app.string-option $ %{} 'FileEntry
      :defs $ {}
        'first-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn first-text (text) (first text)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Option 'String
          :tests $ [] $ %{} 'TestEntry (:name |empty-and-unicode)
            :code $ quote $ let ()
              assert= (Option :none) (first-text |)
              assert= (Option :some "|🌱") (first-text "|🌱树")
              assert= (Option :some |a) (first-text |abc)
            :tags $ #{} :string-option-presence :unit
        'get-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn get-text (text index) (get text index)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String 'Number
            :return $ :: 'Option 'String
          :tests $ [] $ %{} 'TestEntry (:name |bounds-and-unicode)
            :code $ quote $ let ()
              assert= (Option :none) (get-text | 0)
              assert= (Option :none) (get-text "|树🌱" -1)
              assert= (Option :none) (get-text "|树🌱" 2)
              assert= (Option :some "|树") (get-text "|树🌱" 0)
              assert= (Option :some "|🌱") (get-text "|树🌱" 1)
            :tags $ #{} :string-option-presence :unit
        'last-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn last-text (text) (last text)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Option 'String
          :tests $ [] $ %{} 'TestEntry (:name |empty-and-unicode)
            :code $ quote $ let ()
              assert= (Option :none) (last-text |)
              assert= (Option :some "|🌱") (last-text "|树🌱")
              assert= (Option :some |c) (last-text |abc)
            :tags $ #{} :string-option-presence :unit
        'list-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn list-first (items) (first items)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'T
            :generics $ [] 'T
            :return $ :: 'Option 'T
          :tests $ [] $ %{} 'TestEntry (:name |preserves-present-nil)
            :code $ quote $ let ()
              assert= (Option :some nil)
                list-first $ [] nil
              assert= (Option :none)
                list-first $ []
            :tags $ #{} :string-option-presence :unit
        'nth-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nth-text (text index) (nth text index)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String 'Number
            :return $ :: 'Option 'String
          :tests $ []
            %{} 'TestEntry (:name |bounds-and-unicode)
              :code $ quote $ let ()
                assert= (Option :none) (nth-text | 0)
                assert= (Option :none) (nth-text "|树🌱" -1)
                assert= (Option :none) (nth-text "|树🌱" 2)
                assert= (Option :some "|树") (nth-text "|树🌱" 0)
                assert= (Option :some "|🌱") (nth-text "|树🌱" 1)
              :tags $ #{} :string-option-presence :unit
            %{} 'TestEntry (:name |evaluates-arguments-once-in-order)
              :code $ quote $ let
                  calls $ ref 0
                  result $ nth
                    let ()
                      reset! calls $ inc $ deref calls
                      , "|树🌱"
                    let ()
                      assert= 1 $ deref calls
                      reset! calls $ inc $ deref calls
                      , 1
                assert= (Option :some "|🌱") result
                assert= 2 $ deref calls
              :tags $ #{} :string-option-effects :unit
        'sort-date-strings $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sort-date-strings (dates)
            sort dates $ fn (x y)
              &compare
                option:unwrap-or (first y) |
                option:unwrap-or (first x) |
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'String
            :return $ :: 'List 'String
          :tests $ [] $ %{} 'TestEntry (:name |sorts-present-and-empty)
            :code $ quote $ assert= ([] |2026-10-12 |1999-01-01 |)
              sort-date-strings $ [] | |1999-01-01 |2026-10-12
            :tags $ #{} :string-option-presence :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.string-option
    'app.values $ %{} 'FileEntry
      :defs $ {}
        'ClientProjection $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ClientProjection (:count 'Number) (:color 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'LoginState $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LoginState (:username 'String) (:password 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'PatchCandidate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct PatchCandidate
            :raw $ :: 'Map 'Tag 'Dynamic
            :typed 'app.values/ClientProjection
          :examples $ []
          :schema $ :: 'StructDef
        'initial-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def initial-state (LoginState :username | :password |)
          :examples $ []
          :schema $ :: 'app.values/LoginState
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.values
