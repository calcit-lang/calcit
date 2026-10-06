
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-struct
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-struct.main/main!) (:mode :native) (:reload-fn 'test-struct.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {} $ 'test-struct.main
    %{} 'FileEntry
      :defs $ {}
        'A $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct A (:a 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'A0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct A0 (:name 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'B $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct B (:b 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'BirdImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl BirdImpl BirdTrait
            .show $ fn (self)
              println $ &struct:get self :name
            .rename $ fn (self name) (assoc self :name name)
          :examples $ []
          :schema $ :: 'Impl
        'BirdShape $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct BirdShape (:show 'Fn) (:rename 'Fn)
          :examples $ []
          :schema $ :: 'StructDef
        'BirdTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait BirdTrait (.show :fn) (.rename :fn)
          :examples $ []
          :schema $ :: 'Trait
        'C $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct C (:c 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'Cat $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Cat (:name 'String) (:color 'Tag)
          :examples $ []
          :schema $ :: 'StructDef
        'City $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct City (:name 'String) (:province 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'ContextBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ContextBox ([] 'T) (:value 'T)
            :count $ :: 'Option 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'Demo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Demo (:a 'Dynamic) (:b 'Dynamic) (:c 'Dynamic) (:d 'Dynamic)
          :examples $ []
          :schema $ :: 'StructDef
        'Lagopus $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Lagopus (impl-traits Lagopus0 BirdImpl)
          :examples $ []
          :schema $ :: 'Dynamic
        'Lagopus0 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Lagopus0
            :name $ :: 'Optional 'String
          :examples $ []
          :schema $ :: 'StructDef
        'MapLiteralStore $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct MapLiteralStore
            {} $ :text 'String
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ [] $ %{} 'TestEntry (:name |map-literal-fields)
            :code $ quote $ let
                store $ MapLiteralStore :text |ok
              assert= |ok $ :text store
        'NullableEvent $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def NullableEvent
            fn (value) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number
        'NullableEventStore $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NullableEventStore
            :handlers $ :: Map Tag $ :: JsNullish test-struct.main/NullableEvent
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ []
            %{} 'TestEntry (:name |named-callback-members)
              :code $ quote $ let
                  handlers $ {} $ :click NullableEvent
                  stored $ NullableEventStore :handlers handlers
                assert= handlers $ :handlers stored
              :tags $ #{} :js-nullish-container
            %{} 'TestEntry (:name |typed-callback-members)
              :code $ quote $ let
                  callback $ fn (value)
                    hint-fn $ {}
                      :args $ [] 'Number
                      :return 'Unit
                    , &unit
                  handlers $ {} $ :click callback
                  stored $ NullableEventStore :handlers handlers
                assert= handlers $ :handlers stored
              :tags $ #{} :js-nullish-container
            %{} 'TestEntry (:name |wider-callback-input)
              :code $ quote $ let
                  callback $ fn (value)
                    hint-fn $ {}
                      :args $ [] $ :: JsNullish Number
                      :return Unit
                    , &unit
                  handlers $ {} $ :click callback
                  stored $ NullableEventStore :handlers handlers
                assert= handlers $ :handlers stored
              :tags $ #{} :js-nullish-container
            %{} 'TestEntry (:name |mixed-callback-members)
              :code $ quote $ let
                  callback $ fn (value)
                    hint-fn $ {}
                      :args $ [] Number
                      :return Unit
                    , &unit
                  stored $ NullableEventStore :handlers $ {} (:click callback) (:focus nil)
                assert=
                  {} (:click callback) (:focus nil)
                  :handlers stored
              :tags $ #{} :collection-proof
            %{} 'TestEntry (:name |reversed-callback-members)
              :code $ quote $ let
                  stored $ NullableEventStore :handlers $ {} (:focus nil) (:click NullableEvent)
                assert=
                  {} (:focus nil) (:click NullableEvent)
                  :handlers stored
              :tags $ #{} :collection-proof
        'NullableLiteralStore $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NullableLiteralStore
            :items $ :: List $ :: JsNullish Number
            :unique $ :: Set $ :: JsNullish Number
            :nested $ :: Map Tag $ :: List
              :: Map Tag $ :: JsNullish Number
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ [] $ %{} 'TestEntry (:name |nested-literal-members)
            :code $ quote $ let
                stored $ NullableLiteralStore :items ([] 7 nil) :unique (#{} nil 7) :nested $ {}
                  :group $ [] $ {} (:present 7) (:absent nil)
              assert= ([] 7 nil) (:items stored)
              assert= (#{} nil 7) (:unique stored)
              assert=
                {} $ :group $ []
                  {} (:present 7) (:absent nil)
                :nested stored
            :tags $ #{} :collection-proof
        'NullableNumberStore $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NullableNumberStore
            :values $ :: Map Tag $ :: JsNullish Number
            :nested $ :: List $ :: Map Tag (:: JsNullish Number)
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ []
            %{} 'TestEntry (:name |concrete-members)
              :code $ quote $ let
                  numbers $ {} $ :a 1
                  stored $ NullableNumberStore :values numbers :nested $ [] numbers
                assert= numbers $ :values stored
                assert= ([] numbers) (:nested stored)
              :tags $ #{} :js-nullish-container
            %{} 'TestEntry (:name |nil-members)
              :code $ quote $ let
                  absent $ {} $ :a nil
                  stored $ NullableNumberStore :values absent :nested $ [] absent
                assert= absent $ :values stored
                assert= ([] absent) (:nested stored)
              :tags $ #{} :js-nullish-container
            %{} 'TestEntry (:name |mixed-literal-members)
              :code $ quote $ let
                  stored $ NullableNumberStore :values
                    {} (:present 7) (:absent nil)
                    , :nested $ []
                      {} (:absent nil) (:present 7)
                assert=
                  {} (:present 7) (:absent nil)
                  :values stored
                assert=
                  [] $ {} (:absent nil) (:present 7)
                  :nested stored
              :tags $ #{} :collection-proof
        'OptionalFields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptionalFields
            :count $ :: 'Optional 'Number
            :label $ :: 'Optional 'String
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ [] $ %{} 'TestEntry (:name |direct-constructor-admits-concrete-and-nil)
            :code $ quote $ do
              assert= 0 $ :count $ OptionalFields :count 0 :label nil
              assert= nil $ :count $ OptionalFields :count nil :label |saved
            :tags $ #{} :optional-proof :unit
        'OptionalGeneric $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptionalGeneric ([] 'T)
            :value $ :: 'Optional 'T
          :examples $ []
          :schema $ :: 'StructDef
        'Person $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Person
            :name $ :: 'Optional 'String
            :age $ :: 'Optional 'Number
            :position $ :: 'Optional 'Tag
          :examples $ []
          :schema $ :: 'StructDef
        'Point2D $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point2D (:x 'Number) (:y 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'check-point-type $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-point-type (p) (struct? p)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'test-struct.main/Point2D
        'checked-literal-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-literal-alias ()
            let
                values $ assert-type
                  {} (:present 7) (:absent nil)
                  :: 'Map 'Tag $ :: 'JsNullish 'Number
                alias values
              NullableNumberStore :values alias :nested $ [] alias
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'test-struct.main/NullableNumberStore)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |literal-contract)
            :code $ quote $ let
                stored $ checked-literal-alias
              assert=
                {} (:present 7) (:absent nil)
                :values stored
              assert=
                [] $ {} (:present 7) (:absent nil)
                :nested stored
            :tags $ #{} :collection-proof
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (test-struct) (test-methods) (test-match) (test-polymorphism) (test-edn) (test-struct-with) (test-loose-struct-rewrite) (test-map-to-struct) (test-postfix) (do true)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'nullable-callback-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-callback-choice (present?) (if present? NullableEvent nil)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool
            :return $ :: 'JsNullish 'test-struct.main/NullableEvent
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= NullableEvent $ nullable-callback-choice true
              assert= nil $ nullable-callback-choice false
            :tags $ #{} :contextual-proof
        'nullable-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-choice (present? value) (if present? value nil)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= 7 $ nullable-choice true 7
              assert= nil $ nullable-choice false 7
            :tags $ #{} :contextual-proof
        'nullable-implicit-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-implicit-choice (present? value) (if present? value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= 7 $ nullable-implicit-choice true 7
              assert= nil $ nullable-implicit-choice false 7
            :tags $ #{} :contextual-proof
        'nullable-let-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-let-choice (present? value)
            let
                selected $ if present? value nil
                alias selected
              , alias
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= 7 $ nullable-let-choice true 7
              assert= nil $ nullable-let-choice false 7
            :tags $ #{} :contextual-proof
        'nullable-literal-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-literal-return (present?)
            let
                values $ {} (:present 7) (:absent nil)
                alias values
              if present? alias $ {} $ :absent nil
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool
            :return $ :: 'Map 'Tag $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |literal-contract)
            :code $ quote $ let ()
              assert=
                {} (:present 7) (:absent nil)
                nullable-literal-return true
              assert=
                {} $ :absent nil
                nullable-literal-return false
            :tags $ #{} :collection-proof
        'nullable-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-match (selected)
            match selected
              (:some value) value
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= 7 $ nullable-match $ Option :some 7
              assert= nil $ nullable-match $ Option :none
            :tags $ #{} :contextual-proof
        'nullable-raised-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-raised-choice (present? value)
            if present? value $ raise |no-value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ assert= 7 (nullable-raised-choice true 7)
            :tags $ #{} :contextual-proof
        'nullable-reversed-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-reversed-choice (present? value) (if present? nil value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= nil $ nullable-reversed-choice true 7
              assert= 7 $ nullable-reversed-choice false 7
            :tags $ #{} :contextual-proof
        'nullable-shadow-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullable-shadow-choice (present? value)
            let
                selected $ if present? value nil
              let
                  selected selected
                , selected
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool 'Number
            :return $ :: 'JsNullish 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nullable-exit-evidence)
            :code $ quote $ let ()
              assert= 7 $ nullable-shadow-choice true 7
              assert= nil $ nullable-shadow-choice false 7
            :tags $ #{} :contextual-proof
        'nullish-literal-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nullish-literal-list () ([] 7 nil)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'JsNullish $ :: 'List (:: 'JsNullish 'Number)
          :tests $ [] $ %{} 'TestEntry (:name |literal-contract)
            :code $ quote $ assert= ([] 7 nil) (nullish-literal-list)
            :tags $ #{} :collection-proof
        'read-asserted-map-literal-store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-asserted-map-literal-store (source)
            let
                store source
              assert-type store test-struct.main/MapLiteralStore
              :text store
          :examples $ []
          :schema $ :: 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |assert-type-statement-narrows-struct)
            :code $ quote $ assert= |ok
              read-asserted-map-literal-store $ MapLiteralStore :text |ok
        'read-context-box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-context-box (box)
            .unwrap-or (:count box) 160
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'test-struct.main/ContextBox 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |generic-contextual-constructors)
            :code $ quote $ let ()
              assert= 7 $ read-context-box $ ContextBox :value 1 :count (Option :some 7)
              assert= 8 $ read-context-box $ {} (:value |text)
                :count $ Option :some 8
              assert= 160 $ read-context-box $ {} (:value 1)
                :count $ Option :none
            :tags $ #{} :nominal-contextual
        'read-let-asserted-map-literal-store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-let-asserted-map-literal-store (source)
            let
                store $ assert-type source test-struct.main/MapLiteralStore
              :text store
          :examples $ []
          :schema $ :: 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |assert-type-expression-narrows-struct)
            :code $ quote $ assert= |ok
              read-let-asserted-map-literal-store $ MapLiteralStore :text |ok
        'read-number-box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-number-box (box)
            + (:value box)
              .unwrap-or (:count box) 160
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'test-struct.main/ContextBox 'Number
          :tests $ [] $ %{} 'TestEntry (:name |concrete-contextual-constructors)
            :code $ quote $ let ()
              assert= 9 $ read-number-box $ ContextBox :value 1 :count (Option :some 8)
              assert= 9 $ read-number-box $ {} (:value 1)
                :count $ Option :some 8
            :tags $ #{} :nominal-contextual
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reloaded)
          :examples $ []
          :schema $ :: 'Dynamic
        'set-optional-fields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn set-optional-fields (amount label)
            struct-with (OptionalFields :count nil :label nil) (:count amount) (:label label)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'test-struct.main/OptionalFields)
            :args $ [] 'Number 'String
          :tests $ []
            %{} 'TestEntry (:name |concrete-fields-enter-optional)
              :code $ quote $ assert= (OptionalFields :count 7 :label |saved) (set-optional-fields 7 |saved)
              :tags $ #{} :optional-proof :unit
            %{} 'TestEntry (:name |zero-and-empty-string-remain-values)
              :code $ quote $ assert= (OptionalFields :count 0 :label |) (set-optional-fields 0 |)
              :tags $ #{} :optional-proof :unit
            %{} 'TestEntry (:name |nil-writes-remain-valid)
              :code $ quote $ assert= (OptionalFields :count nil :label nil)
                struct-with (set-optional-fields 7 |saved) (:count nil) (:label nil)
              :tags $ #{} :optional-proof :unit
        'set-optional-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn set-optional-generic (record value)
            struct-with record $ :value value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] (:: 'test-struct.main/OptionalGeneric 'T) 'T
            :generics $ [] 'T
            :return $ :: 'test-struct.main/OptionalGeneric 'T
          :tests $ [] $ %{} 'TestEntry (:name |preserves-generic-payload-evidence)
            :code $ quote $ do
              assert= (OptionalGeneric :value 9)
                set-optional-generic (OptionalGeneric :value 1) 9
              assert= (OptionalGeneric :value |)
                set-optional-generic (OptionalGeneric :value |saved) |
            :tags $ #{} :optional-proof :unit
        'sum-point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sum-point (p)
            &+ (:x p) (:y p)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'test-struct.main/Point2D
        'test-edn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn ()
            let
                content "|%{} :Lagopus0 (:name |La)"
                data $ parse-cirru-edn content $ {}
                  :Lagopus0 $ %{} Lagopus $ :name nil
              println |EDN: data
              assert= true $ any? (&struct:impls data)
                fn (impl)
                  = (impl-origin impl) (%some BirdTrait)
            let
                l1 $ %{} Lagopus $ :name |LagopusA
              println |EDN: $ format-cirru-edn l1
            let
                data $ %{} Demo (:a 1)
                  :b $ [] 2 3
                  :c 4
                  :d 5
              assert= "|%{} 'Demo (:a 1) (:c 4) (:d 5)\n  :b $ [] 2 3" $ trim $ format-cirru-edn data
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-loose-struct-rewrite $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing loose-to-struct rewrite")
            assert= 30 $ sum-point $ ?{} :x 10 :y 20
            assert= true $ check-point-type $ ?{} :x 10 :y 20
          :examples $ []
          :schema $ :: 'Dynamic
        'test-map-to-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing map-to-struct rewrite")
            assert= 30 $ sum-point $ {} (:x 10) (:y 20)
            assert= true $ check-point-type $ {} (:x 10) (:y 20)
          :examples $ []
          :schema $ :: 'Dynamic
        'test-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing struct match")
            let
                a1 $ %{} A $ :a 1
                b1 $ %{} B $ :b 2
                c1 $ %{} C $ :c 3
              assert= 1 $ struct-match a1
                A aa $ :a aa
                B bb $ :b bb
                _ o (println |others) :other
              assert= 2 $ struct-match b1
                A aa $ :a aa
                B bb $ :b bb
                _ o (println |others) :other
              assert= :other $ struct-match c1
                A aa $ :a aa
                B bb $ :b bb
                _ o (println |others) :other
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing struct methods")
            &let
              kitty $ %{} Cat (:name |kitty) (:color :red)
              assert= :Cat $ &struct:get-name kitty
              assert= :red $ :color kitty
              assert= true $ = (&struct:definition kitty) Cat
              assert= true $ struct-def? $ &struct:definition kitty
              assert= true $ &struct:matches? kitty $ %{} (&struct:definition kitty) (:name |kitty) (:color :red)
              assert= (&struct:to-map kitty) (&{} :name |kitty :color :red)
              assert= 2 $ &struct:count kitty
              assert= true $ &struct:contains? kitty $ &struct:field-tag kitty 0
              assert= true $ &struct:contains? kitty $ &struct:field-tag kitty 1
              assert= true $ &struct:contains? kitty :color
              assert= false $ &struct:contains? kitty :age
              assert=
                %{} Cat (:name |kitty) (:color :blue)
                &struct:assoc kitty :color :blue
              assert=
                &struct:from-map Cat $ &{} :name |kitty :color :red
                %{} Cat (:name |kitty) (:color :red)
              &let
                persian $ &struct:extend-as kitty :Persian :age 10
                assert= 10 $ &struct:nth persian 0 :age
                assert= :Persian $ &struct:get-name persian
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-polymorphism $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Test struct polymorphism") (println Lagopus)
            let
                l1 $ %{} Lagopus $ :name |LagopusA
                a1 A0
                a2 $ impl-traits a1 BirdImpl
                a1r $ %{} a2 $ :name |Demo
                l1t l1
              assert-traits l1t BirdTrait
              let
                  l2 $ l1t .rename |LagopusB
                  l2t l2
                assert-traits l2t BirdTrait
                println l1
                l1t .show
                l2t .show
                assert= (&struct:impls l1) (&struct:impls a1r)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-postfix $ %{} 'CodeEntry (:doc "|test postfix syntax")
          :code $ quote $ fn () (log-title "|Testing postfix syntax")
            let
                p $ &%{} Point2D :x 10 :y 20
              assert= 10 $ p :x
              assert= 20 $ p :y
            let
                ffi-point $ unsafe-coerce (?{} :x 30 :y 40) Point2D
              assert= 30 $ ffi-point :x
              assert= 40 $ ffi-point :y
            let
                l1 $ %{} Lagopus $ :name |LagopusA
              assert= |LagopusA $ :name l1
              let
                  l2 $ l1 .rename |LagopusB
                assert-type l2 Lagopus
                assert= |LagopusB $ :name l2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn () (log-title "|Testing struct")
            let
                p1 $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
                p2 $ &%{} Person :name |Chen :age 20 :position :mainland
                p0 $ &%{} Person :name nil :age nil :position nil
                p3 $ &%{} Person :name |Chen :age 23 :position :mainland
                c1 $ %{} City (:name |Shanghai) (:province |Shanghai)
              assert= true $ = (&struct:definition p0) Person
              assert= nil $ :age p0
              assert= nil $ :name p0
              assert= nil $ :position p0
              assert= 20 $ :age p1
              assert= 20 $ :age p2
              assert= 23 $ :age p3
              assert= 23 $ :age p3
              assert= :struct $ type-of p1
              assert= (&struct:to-map p1)
                {} (:name |Chen) (:age 20) (:position :mainland)
              assert= 21 $ :age $ &struct:from-map Person
                {} (:name |Chen) (:age 21) (:position :mainland)
              assert=
                .keys $ .to-map p2
                #{} :age :name :position
              assert-detect identity $ &struct:matches? p1 p1
              assert-detect identity $ &struct:matches? p1 p2
              assert-detect not $ &struct:matches? p1 c1
              &let
                p4 $ assoc p1 :age 30
                assert= 20 $ :age p1
                assert= 30 $ :age p4
              inside-js: $ js/console.log $ to-js-data p1
              assert-detect identity $ = p1 p1
              assert-detect identity $ = p1 p2
              assert-detect not $ = p1 p3
              assert-detect not $ &= p1 c1
              assert=
                %{} Person (:age 23) (:name |Ye) (:position :mainland)
                struct-with p1 (:age 23) (:name |Ye)
              assert=
                %{} Person (:age 23) (:name |Ye) (:position :mainland)
                struct-with p1 (:age 23) (:name |Ye)
              assert-detect identity $ contains? p1 :name
              assert-detect not $ contains? p1 :surname
              assert= 3 $ count p1
              assert= 21 $ :age $ assoc p1 :age 21
              assert= 20 $ :age p1
              let
                  read-open $ fn (value)
                    hint-fn $ {}
                      :args $ [] 'Dynamic
                      :return $ :: 'List $ :: 'Option 'Dynamic
                    [] (get value :age) (get value |name) (get value :missing) (get value 0)
                assert=
                  [] (%some 20) (%some |Chen) (%none) (%none)
                  read-open p1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
            :features $ #{} :js-ffi
        'test-struct-complete-construction $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-struct-complete-construction ()
            let
                Shape Point2D
                complete $ %{} Point2D (:y 2) (:x 1)
                alias $ %{} Shape (:x 1) (:y 2)
                dynamic $ fn (prototype)
                  hint-fn $ {}
                    :args $ [] 'StructDef
                    :return 'Dynamic
                  %{} prototype (:x 1) (:y 2)
              assert= complete alias
              assert= complete $ dynamic Point2D
              , 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |complete-alias-dynamic)
              :code $ quote $ assert= 1 (test-struct-complete-construction)
              :tags $ #{} :struct-shape :unit
            %{} 'TestEntry (:name |string-field-names)
              :code $ quote $ assert= (Point2D :x 1 :y 2)
                %{} Point2D (|x 1) (|y 2)
              :tags $ #{} :struct-field-names :unit
            %{} 'TestEntry (:name |symbol-field-names)
              :code $ quote $ assert= (Point2D :x 1 :y 2)
                %{} Point2D ('x 1) ('y 2)
              :tags $ #{} :struct-field-names :unit
            %{} 'TestEntry (:name |runtime-field-names)
              :code $ quote $ let
                  x-name |x
                  y-name |y
                assert= (Point2D :x 1 :y 2)
                  %{} Point2D (x-name 1) (y-name 2)
              :tags $ #{} :struct-field-names :unit
            %{} 'TestEntry (:name |runtime-duplicate-field-names)
              :code $ quote $ let
                  field-name |x
                assert= :rejected $ try
                  do
                    %{} Point2D (field-name 1) (field-name 2)
                    , :accepted
                  fn (error) :rejected
              :tags $ #{} :struct-field-names :unit
        'test-struct-with $ %{} 'CodeEntry (:doc "|test struct-with")
          :code $ quote $ fn () (log-title "|Testing struct-with")
            let
                p1 $ %{} Person (:name |Chen) (:age 20) (:position :hangzhou)
                p2 $ struct-with p1 (:age 21) (:position :shanghai)
              ; println |P2 p2
              assert= 20 $ :age p1
              assert= 21 $ :age p2
              assert= :hangzhou $ :position p1
              assert= :shanghai $ :position p2
              assert= |Chen $ :name p2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'try-bool $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-bool (fail?)
            try
              if fail? (raise |fixture-failure) true
              fn (message) false
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal-bool)
              :code $ quote $ assert= true (try-bool false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught-bool)
              :code $ quote $ assert= false (try-bool true)
              :tags $ #{} :try-proof :unit
        'try-effect-order $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-effect-order (fail?)
            let
                events $ atom $ [] |start
              try
                let ()
                  reset! events $ .append (deref events) |body
                  if fail? (raise |fixture-failure) 7
                let ()
                  reset! events $ .append (deref events) |handler-eval
                  fn (message) (assert= |fixture-failure message)
                    reset! events $ .append (deref events) |handler-call
                    , 0
              deref events
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool
            :return $ :: 'List 'String
          :tests $ []
            %{} 'TestEntry (:name |normal-evaluation-order)
              :code $ quote $ assert= ([] |start |body) (try-effect-order false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught-evaluation-order)
              :code $ quote $ assert= ([] |start |body |handler-eval |handler-call) (try-effect-order true)
              :tags $ #{} :try-proof :unit
        'try-factory-never $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-factory-never (fail?)
            try
              if fail? (raise |fixture-failure) 7
              raise |handler-failure
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal-does-not-evaluate-raising-factory)
              :code $ quote $ assert= 7 (try-factory-never false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |raising-factory-failure-propagates)
              :code $ quote $ assert= |handler-failure
                try (try-factory-never true)
                  fn (message) message
              :tags $ #{} :try-proof :unit
        'try-handler-never $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-handler-never (fail?)
            try
              if fail? (raise |fixture-failure) 7
              fn (message) (raise message)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |raise-has-no-caught-value)
              :code $ quote $ assert= 7 (try-handler-never false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |handler-failure-propagates)
              :code $ quote $ assert= |fixture-failure
                try (try-handler-never true)
                  fn (message) message
              :tags $ #{} :try-proof :unit
        'try-hinted-handler $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-hinted-handler (fail?)
            try
              if fail? (raise |fixture-failure) true
              fn (message)
                hint-fn $ {}
                  :args $ [] 'String
                  :return 'Bool
                , false
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal)
              :code $ quote $ assert= true (try-hinted-handler false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught)
              :code $ quote $ assert= false (try-hinted-handler true)
              :tags $ #{} :try-proof :unit
        'try-imported-handler $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-imported-handler (fail?)
            try
              if fail? (raise |fixture-failure) |normal
              , identity
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal)
              :code $ quote $ assert= |normal (try-imported-handler false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught)
              :code $ quote $ assert= |fixture-failure (try-imported-handler true)
              :tags $ #{} :try-proof :unit
        'try-lazy! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-lazy! ()
            let
                counter $ atom 0
              assert= 7 $ try 7 $ let () (reset! counter 1)
                fn (message) 0
              deref counter
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |handler-factory-is-lazy)
            :code $ quote $ assert= 0 (try-lazy!)
            :tags $ #{} :try-proof :unit
        'try-normal-never $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-normal-never ()
            try (raise |fixture-failure)
              fn (message) message
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |raise-has-no-normal-value)
            :code $ quote $ assert= |fixture-failure (try-normal-never)
            :tags $ #{} :try-proof :unit
        'try-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-option (fail?)
            try
              if fail? (raise |fixture-failure) (Option :some 7)
              fn (message) (Option :none)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool
            :return $ :: 'Option 'Number
          :tests $ []
            %{} 'TestEntry (:name |normal-option)
              :code $ quote $ assert= (Option :some 7) (try-option false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught-option)
              :code $ quote $ assert= (Option :none) (try-option true)
              :tags $ #{} :try-proof :unit
        'try-optional-handler $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-optional-handler (fail?)
            try
              if fail? (raise |fixture-failure) |normal
              fn (message extra)
                hint-fn $ {}
                  :args $ [] 'String $ :: 'Option 'Number
                  :return 'String
                assert= (Option :none) extra
                , message
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal-value)
              :code $ quote $ assert= |normal (try-optional-handler false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |omitted-trailing-option-is-none)
              :code $ quote $ assert= |fixture-failure (try-optional-handler true)
              :tags $ #{} :try-proof :unit
        'try-proc-handler $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-proc-handler (fail?)
            try
              if fail? (raise |fixture-failure) 7
              , &str:count
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal)
              :code $ quote $ assert= 7 (try-proc-handler false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught)
              :code $ quote $ assert= 15 (try-proc-handler true)
              :tags $ #{} :try-proof :unit
        'try-rest-handler $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-rest-handler (fail?)
            try
              if fail? (raise |fixture-failure) 7
              fn (& messages)
                hint-fn $ {}
                  :args $ []
                  :rest 'String
                  :return 'Number
                .count messages
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal-value)
              :code $ quote $ assert= 7 (try-rest-handler false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |handler-receives-one-string-in-list)
              :code $ quote $ assert= 1 (try-rest-handler true)
              :tags $ #{} :try-proof :unit
        'try-rest-ignored $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-rest-ignored (fail?)
            try
              if fail? (raise |fixture-failure) 7
              fn (& messages) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal)
              :code $ quote $ assert= 7 (try-rest-ignored false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught)
              :code $ quote $ assert= 0 (try-rest-ignored true)
              :tags $ #{} :try-proof :unit
        'try-rest-prefix $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-rest-prefix (fail?)
            try
              if fail? (raise |fixture-failure) 7
              fn (message & others) (.count message)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal)
              :code $ quote $ assert= 7 (try-rest-prefix false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught)
              :code $ quote $ assert= 15 (try-rest-prefix true)
              :tags $ #{} :try-proof :unit
        'try-result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-result (fail?)
            try
              if fail? (raise |fixture-failure) (Result :ok 7)
              fn (message) (Result :err message)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Bool
            :return $ :: 'Result 'Number 'String
          :tests $ []
            %{} 'TestEntry (:name |normal-result)
              :code $ quote $ assert= (Result :ok 7) (try-result false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught-result)
              :code $ quote $ assert= (Result :err |fixture-failure) (try-result true)
              :tags $ #{} :try-proof :unit
        'try-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-string (fail?)
            try
              if fail? (raise |fixture-failure) |normal
              fn (message) message
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |normal-string)
              :code $ quote $ assert= |normal (try-string false)
              :tags $ #{} :try-proof :unit
            %{} 'TestEntry (:name |caught-string-input)
              :code $ quote $ assert= |fixture-failure (try-string true)
              :tags $ #{} :try-proof :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-struct.main
          :require $ util.core :refer $ log-title inside-js:
