
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-traits
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-traits.main/main!) (:mode :native) (:reload-fn 'test-traits.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-traits.external $ %{} 'FileEntry
      :defs $ {}
        'Counter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Counter
            impl-traits
              defstruct Counter $ :value 'Number
              , CounterOpsImpl CounterAddImpl
          :examples $ []
          :schema $ :: 'StructDef
        'CounterAddImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl CounterAddImpl Add (.add counter:add)
          :examples $ []
          :schema $ :: 'Impl
        'CounterOps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait CounterOps
            .next $ :: 'Fn $ {}
              :args $ [] 'test-traits.external/Counter
              :return 'test-traits.external/Counter
          :examples $ [] $ quote CounterOps
          :schema $ :: 'Trait
        'CounterOpsImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl CounterOpsImpl CounterOps (.next counter:next)
          :examples $ []
          :schema $ :: 'Impl
        'counter:add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn counter:add (self other)
            %{} Counter $ :value $ + (:value self) (:value other)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'test-traits.external/Counter)
            :args $ [] 'test-traits.external/Counter 'test-traits.external/Counter
        'counter:next $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn counter:next (self)
            %{} Counter $ :value $ inc (:value self)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'test-traits.external/Counter)
            :args $ [] 'test-traits.external/Counter
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.external
    'test-traits.identity $ %{} 'FileEntry
      :defs $ {} $ 'make-marker
        %{} 'CodeEntry
          :doc "|Create a fresh nominal marker for shared equality, hashing and ordering regressions."
          :code $ quote $ defn make-marker () (deftrait Marker)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Trait)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |keeps-aliases-and-fresh-traits-distinct)
              :code $ quote $ let
                  left $ make-marker
                  right $ make-marker
                  alias left
                assert= true $ = left alias
                assert= false $ = left right
                assert= false $ = right left
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |orders-by-creation-not-comparison-history)
              :code $ quote $ let
                  left $ make-marker
                  right $ make-marker
                assert= 1 $ &compare right left
                assert= -1 $ &compare left right
                assert= 0 $ &compare left left
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |keeps-nominal-map-and-set-keys)
              :code $ quote $ let
                  left $ make-marker
                  right $ make-marker
                  values $ {} (left |left) (right |right)
                  origins $ #{} left right left
                assert= 2 $ count values
                assert= (Option :some |left) (.get values left)
                assert= (Option :some |right) (.get values right)
                assert= 2 $ count origins
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |keeps-impl-origins-nominal)
              :code $ quote $ let
                  left $ make-marker
                  right $ make-marker
                  left-impl $ defimpl MarkerImpl left
                  same-impl $ defimpl MarkerImpl left
                  right-impl $ defimpl MarkerImpl right
                  Item $ defenum Item $ :item
                assert= true $ = left-impl same-impl
                assert= false $ = left-impl right-impl
                assert= 0 $ &compare left-impl same-impl
                assert= 1 $ &compare right-impl left-impl
                assert= true $ = (impl-traits Item left-impl) (impl-traits Item same-impl)
                assert= false $ = (impl-traits Item left-impl) (impl-traits Item right-impl)
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |orders-definition-kinds-like-native)
              :code $ quote $ let
                  NominalRecord $ defstruct NominalRecord $ :value 'Number
                  Choice $ defenum Choice $ :choice
                  marker $ make-marker
                  implementation $ defimpl MarkerImpl marker
                assert= -1 $ &compare NominalRecord Choice
                assert= -1 $ &compare Choice marker
                assert= -1 $ &compare marker implementation
                assert= 1 $ &compare implementation marker
              :tags $ #{} :trait :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.identity
    'test-traits.main $ %{} 'FileEntry
      :defs $ {}
        'CoreShowImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl CoreShowImpl calcit.core/Show (.show core-show:show)
          :examples $ []
          :schema $ :: 'Impl
        'CoreToStringImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl CoreToStringImpl calcit.core/ToString (.to-string core-show:show)
          :examples $ []
          :schema $ :: 'Impl
        'Demo0 $ %{} 'CodeEntry (:doc "|Enum prototype for tuple trait tests")
          :code $ quote $ defenum Demo (:demo 'Dynamic)
          :examples $ []
          :schema $ :: 'Enum
        'DemoBar $ %{} 'CodeEntry (:doc "|Enum with MyBar impls")
          :code $ quote $ def DemoBar (impl-traits Demo0 MyBarImpl2)
          :examples $ []
          :schema $ :: 'EnumDef
        'DemoZap $ %{} 'CodeEntry (:doc "|Enum with MyZapA/MyZapB")
          :code $ quote $ def DemoZap (impl-traits Demo0 MyZapAImpl MyZapBImpl)
          :examples $ []
          :schema $ :: 'EnumDef
        'DemoZapA $ %{} 'CodeEntry (:doc "|Enum with MyZapA then MyZapB")
          :code $ quote $ def DemoZapA (impl-traits Demo0 MyZapAImpl MyZapBImpl)
          :examples $ []
          :schema $ :: 'EnumDef
        'DemoZapB $ %{} 'CodeEntry (:doc "|Enum with MyZapB then MyZapA")
          :code $ quote $ def DemoZapB (impl-traits Demo0 MyZapBImpl MyZapAImpl)
          :examples $ []
          :schema $ :: 'EnumDef
        'Greeter $ %{} 'CodeEntry
          :doc "|Struct carrying both a required trait impl and its child"
          :code $ quote $ def Greeter
            impl-traits
              defstruct Greeter $ :name 'String
              , LabeledImpl GreetingImpl
          :examples $ []
          :schema $ :: 'StructDef
        'Greeting $ %{} 'CodeEntry (:doc "|Trait that requires Labeled")
          :code $ quote $ deftrait Greeting (requires Labeled)
            .greeting $ :: 'Fn $ {} (:return 'String)
              :args $ [] 'test-traits.main/Greeting
          :examples $ []
          :schema $ :: 'Trait
        'GreetingImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl GreetingImpl Greeting
            .greeting $ fn (g)
              str "|hello " $ :name $ assert-type g Greeter
          :examples $ []
          :schema $ :: 'Impl
        'Labeled $ %{} 'CodeEntry (:doc "|Parent trait required by Greeting")
          :code $ quote $ deftrait Labeled
            .label $ :: 'Fn $ {} (:return 'String)
              :args $ [] 'test-traits.main/Labeled
          :examples $ []
          :schema $ :: 'Trait
        'LabeledImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl LabeledImpl Labeled
            .label $ fn (g)
              :name $ assert-type g Greeter
          :examples $ []
          :schema $ :: 'Impl
        'MyBar $ %{} 'CodeEntry (:doc "|Trait for tuple override test")
          :code $ quote $ deftrait MyBar (.bar :fn)
          :examples $ []
          :schema $ :: 'Trait
        'MyBarImpl $ %{} 'CodeEntry (:doc "|Trait impl for tuple override test")
          :code $ quote $ defimpl MyBarImpl MyBar (.bar mybar:bar1)
          :examples $ []
          :schema $ :: 'Impl
        'MyBarImpl2 $ %{} 'CodeEntry (:doc "|Trait impl for tuple override test")
          :code $ quote $ defimpl MyBarImpl2 MyBar (.bar mybar:bar2)
          :examples $ []
          :schema $ :: 'Impl
        'MyFoo $ %{} 'CodeEntry (:doc "|Trait for deftrait test")
          :code $ quote $ deftrait MyFoo (.foo :fn)
          :examples $ []
          :schema $ :: 'Trait
        'MyFooImpl $ %{} 'CodeEntry (:doc "|Trait impl for deftrait test")
          :code $ quote $ defimpl MyFooImpl MyFoo (.foo myfoo:foo)
          :examples $ []
          :schema $ :: 'Impl
        'MyFooImpl2 $ %{} 'CodeEntry (:doc "|Trait impl for override test")
          :code $ quote $ defimpl MyFooImpl2 MyFoo (.foo myfoo:foo2)
          :examples $ []
          :schema $ :: 'Impl
        'MyZapA $ %{} 'CodeEntry (:doc "|Trait A for cross-trait method conflict test")
          :code $ quote $ deftrait MyZapA (.zap :fn)
          :examples $ []
          :schema $ :: 'Trait
        'MyZapAImpl $ %{} 'CodeEntry
          :doc "|Trait A impl for cross-trait method conflict test"
          :code $ quote $ defimpl MyZapAImpl MyZapA (.zap myzap:a)
          :examples $ []
          :schema $ :: 'Impl
        'MyZapB $ %{} 'CodeEntry (:doc "|Trait B for cross-trait method conflict test")
          :code $ quote $ deftrait MyZapB (.zap :fn)
          :examples $ []
          :schema $ :: 'Trait
        'MyZapBImpl $ %{} 'CodeEntry
          :doc "|Trait B impl for cross-trait method conflict test"
          :code $ quote $ defimpl MyZapBImpl MyZapB (.zap myzap:b)
          :examples $ []
          :schema $ :: 'Impl
        'Person0 $ %{} 'CodeEntry (:doc "|Struct used in trait tests")
          :code $ quote $ defstruct Person0 (:name 'String)
          :examples $ []
          :schema $ :: 'Struct
        'RestBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def RestBox (impl-traits RestBoxBase RestBoxImpl)
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ []
            %{} 'TestEntry (:name |direct-method)
              :code $ quote $ assert= 13
                .sum (RestBox :value 10) |numbers 1 2
              :tags $ #{} :unit
            %{} 'TestEntry (:name |explicit-no-rest)
              :code $ quote $ assert= 10
                &trait-call RestBoxOps :sum (RestBox :value 10) |numbers
              :tags $ #{} :unit
            %{} 'TestEntry (:name |explicit-list)
              :code $ quote $ assert= 13
                explicit-rest-sum (RestBox :value 10) ([] 1 2)
              :tags $ #{} :unit
            %{} 'TestEntry (:name |explicit-empty-list)
              :code $ quote $ assert= 10
                explicit-rest-sum (RestBox :value 10) ([])
              :tags $ #{} :unit
            %{} 'TestEntry (:name |method-list)
              :code $ quote $ let
                  items $ [] 1 2
                assert= 13 $ .sum (RestBox :value 10) |numbers & items
              :tags $ #{} :unit
            %{} 'TestEntry (:name |explicit-literal)
              :code $ quote $ assert= 13
                &trait-call RestBoxOps :sum (RestBox :value 10) |numbers & $ [] 1 2
              :tags $ #{} :unit
            %{} 'TestEntry (:name |fixed-method-empty-spread)
              :code $ quote $ assert= 2
                &trait-call Countable :count ([] 1 2) & $ []
              :tags $ #{} :unit
            %{} 'TestEntry (:name |single-evaluation)
              :code $ quote $ let
                  calls $ ref 0
                  supply $ fn ()
                    hint-fn $ {}
                      :args $ []
                      :return $ :: 'List 'Number
                    swap! calls inc
                    [] 1 2
                assert= 13 $ &trait-call RestBoxOps :sum (RestBox :value 10) |numbers & $ supply
                assert= 1 $ deref calls
              :tags $ #{} :unit
        'RestBoxBase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct RestBoxBase (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'RestBoxImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl RestBoxImpl RestBoxOps
            .sum $ fn (self label & values)
              hint-fn $ {}
                :args $ [] 'test-traits.main/RestBox 'String
                :rest 'Number
                :return 'Number
              foldl values (:value self) &+
          :examples $ []
          :schema $ :: 'Impl
        'RestBoxOps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait RestBoxOps
            .sum $ :: 'Fn $ {}
              :args $ [] 'test-traits.main/RestBox 'String
              :rest 'Number
              :return 'Number
          :examples $ []
          :schema $ :: 'Trait
        'ResultBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def ResultBox (impl-traits ResultBoxBase ResultBoxImpl)
          :examples $ []
          :schema $ :: 'StructDef
          :tests $ []
            %{} 'TestEntry (:name |preserves-nested-method-result)
              :code $ quote $ let
                  box $ ResultBox :value 7
                assert= 7 $ .unwrap $ .find box |x
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |preserves-local-method-result)
              :code $ quote $ let
                  box $ ResultBox :value 7
                  found $ .find box |x
                assert= 7 $ found .unwrap
              :tags $ #{} :trait :unit
        'ResultBoxBase $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ResultBoxBase (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'ResultBoxImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl ResultBoxImpl ResultBoxOps
            .find $ fn (self text)
              hint-fn $ {}
                :args $ [] 'test-traits.main/ResultBox 'String
                :return $ :: 'Option 'Number
              Option :some $ :value self
          :examples $ []
          :schema $ :: 'Impl
        'ResultBoxOps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait ResultBoxOps
            .find $ :: 'Fn $ {}
              :args $ [] 'test-traits.main/ResultBox 'String
              :return $ :: 'Option 'Number
          :examples $ []
          :schema $ :: 'Trait
        'add-with-trait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn add-with-trait (a b) (a .add b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Add
          :tests $ []
            %{} 'TestEntry (:name |dispatches-number-and-string)
              :code $ quote $ do
                assert= 3 $ add-with-trait 1 2
                assert= |ab $ add-with-trait |a |b
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |dispatches-custom-add)
              :code $ quote $ let
                  a $ %{} test-traits.external/Counter $ :value 2
                  b $ %{} test-traits.external/Counter $ :value 3
                  summed $ add-with-trait a b
                assert= 5 $ :value summed
              :tags $ #{} :trait :unit
        'compare-with-trait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compare-with-trait (a b) (a .compare b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'T 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Compare
        'contains-with-trait? $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn contains-with-trait? (x k) (x .contains? k)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'T 'K
            :generics $ [] 'T 'K
            :where $ {} $ 'T 'Contains
        'core-show:show $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn core-show:show (p)
            str |Person: $ :name p
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'test-traits.main/Person0
        'count-with-trait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-with-trait (x) (x .count)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Countable
        'describe-greeting $ %{} 'CodeEntry
          :doc "|A Greeting bound also proves the required Labeled methods"
          :code $ quote $ defn describe-greeting (x)
            str (x .greeting) "| / " $ x .label
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Greeting
          :tests $ [] $ %{} 'TestEntry (:name |qualified-trait-receiver)
            :code $ quote $ assert=
              describe-greeting $ Greeter :name |Ada
              , "|hello Ada / Ada"
            :tags $ #{} :trait-bound :unit
        'explicit-rest-sum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn explicit-rest-sum (box items) (&trait-call RestBoxOps :sum box |numbers & items)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'test-traits.main/RestBox $ :: 'List 'Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (&init-builtin-impls!) (println "|Testing built-in traits...")
            ; Test Debug trait - all types should have it
            test-debug-trait
            ; Test deftrait macro
            test-deftrait
            ; Test impl precedence order
            test-impl-precedence-order
            test-enum-impl-precedence-order
            test-cross-trait-method-conflict
            test-explicit-trait-call
            ; Test Eq trait
            test-eq-trait
            ; Test Compare trait
            test-compare-trait
            test-bit-shifts
            ; Test Add trait
            test-add-trait
            ; Test Len/Empty traits
            test-collection-traits
            ; Test Option/Result Mappable
            test-option-result-map
            ; Test assert-traits
            test-assert-trait
            ; Debug helpers: methods introspection
            test-method-introspection
            ; Test requires between ordinary traits
            test-trait-requires
            assert= true $ test-qualified-contains-boundary
            println "|All trait tests passed!"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'mybar:bar1 $ %{} 'CodeEntry (:doc "|method implementation for MyBarImpl/:bar")
          :code $ quote $ defn mybar:bar1 (_x) |bar1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'mybar:bar2 $ %{} 'CodeEntry (:doc "|method implementation for MyBarImpl2/:bar")
          :code $ quote $ defn mybar:bar2 (_x) |bar2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'myfoo:foo $ %{} 'CodeEntry (:doc "|method implementation for MyFoo/:foo")
          :code $ quote $ defn myfoo:foo (p)
            str "|foo " $ :name p
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'test-traits.main/Person0
        'myfoo:foo2 $ %{} 'CodeEntry (:doc "|method implementation for MyFooImpl2/:foo")
          :code $ quote $ defn myfoo:foo2 (p)
            str "|foo2 " $ :name p
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'test-traits.main/Person0
        'myzap:a $ %{} 'CodeEntry (:doc "|method implementation for MyZapA/:zap")
          :code $ quote $ defn myzap:a (_x) |zapA
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'myzap:b $ %{} 'CodeEntry (:doc "|method implementation for MyZapB/:zap")
          :code $ quote $ defn myzap:b (_x) |zapB
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'test-add-trait $ %{} 'CodeEntry (:doc "|Test Add trait")
          :code $ quote $ defn test-add-trait () (println "|Testing Add trait...") (; Number addition)
            assert= 3 $ + 1 2
            assert= 10 $ + 1 2 3 4
            ; String concatenation $ using str
            assert= "|hello world" $ str-spaced |hello |world
            ; List concatenation
            assert= ([] 1 2 3 4)
              &list:concat ([] 1 2) ([] 3 4)
            assert= ([] 1 2)
              .append ([] 1) 2
            assert= 3 $ add-with-trait 1 2
            assert= |ab $ add-with-trait |a |b
            let
                a $ %{} test-traits.external/Counter $ :value 2
                b $ %{} test-traits.external/Counter $ :value 3
              assert= 5 $ :value $ add-with-trait a b
            println "|  Add trait: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-assert-trait $ %{} 'CodeEntry (:doc "|Test assert-traits")
          :code $ quote $ defn test-assert-trait () (println "|Testing assert-traits...")
            let
                x 1
                xs $ [] 1 2 3
                m $ {} (:a 1) (:b 2)
                st $ #{} 1 2 3
                s |hello
                opt $ Option :some 1
                Person $ impl-traits Person0 MyFooImpl
                p $ %{} Person $ :name |Alice
                ZapPerson $ impl-traits Person0 MyZapAImpl
                zp $ %{} ZapPerson $ :name |Bob
                flag true
                keyword :demo
                nothing nil
              assert= x $ assert-traits x calcit.core/Debug
              assert= x $ assert-traits x calcit.core/Debug calcit.core/Eq
              assert= xs $ assert-traits xs calcit.core/Mappable
              assert= xs $ assert-traits xs calcit.core/Mappable calcit.core/Debug
              assert= m $ assert-traits m calcit.core/Mappable
              assert= m $ assert-traits m calcit.core/Mappable calcit.core/Debug
              assert= st $ assert-traits st calcit.core/Mappable
              assert= st $ assert-traits st calcit.core/Mappable calcit.core/Debug
              assert= s $ assert-traits s calcit.core/Debug
              assert= s $ assert-traits s calcit.core/Debug calcit.core/Eq
              assert= opt $ assert-traits opt calcit.core/Mappable
              assert= p $ assert-traits p MyFoo
              ; Structs satisfy the built-in Debug trait through the shared struct impls.
              assert= p $ assert-traits p calcit.core/Debug
              ; Person has no implementation of the unrelated MyBar trait.
              assert= :true $ try
                do (assert-traits p MyBar) :false
                fn (e) (do :true)
              ; A same-named method from MyZapA must not satisfy the distinct MyZapB trait.
              assert= zp $ assert-traits zp MyZapA
              assert= :true $ try
                do (assert-traits zp MyZapB) :false
                fn (e) (do :true)
              assert= flag $ assert-traits flag calcit.core/Debug calcit.core/Eq
              assert= keyword $ assert-traits keyword calcit.core/Debug calcit.core/Eq
              assert= nothing $ assert-traits nothing calcit.core/Debug calcit.core/Eq
            println "|  assert-traits: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-bit-shifts $ %{} 'CodeEntry
          :doc "|Shift counts use their low five bits on every backend"
          :code $ quote $ defn test-bit-shifts ()
            assert= 4 $ bit-shl 1 2
            assert= 1 $ bit-shl 1 32
            assert= 2 $ bit-shl 1 33
            assert= -2147483648 $ bit-shl 1 31
            assert= -2147483648 $ bit-shl 1 -1
            assert= 2 $ bit-shr 8 2
            assert= 8 $ bit-shr 8 32
            assert= -4 $ bit-shr -8 1
            assert= -1 $ bit-shr -8 -1
            assert= 0 $ bit-shr 5 100
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-collection-traits $ %{} 'CodeEntry (:doc "|Test Len/Empty/Contains traits for collections")
          :code $ quote $ defn test-collection-traits () (println "|Testing Collection traits (Len, Empty)...") (; Len trait)
            assert= 0 $ count $ []
            assert= 3 $ count $ [] 1 2 3
            assert= 5 $ count |hello
            assert= 2 $ count $ {} (:a 1) (:b 2)
            assert= 3 $ count $ #{} 1 2 3
            ; Empty trait
            assert= true $ empty? $ []
            assert= false $ empty? $ [] 1
            assert= true $ empty? $ {}
            assert= false $ empty? $ {} (:a 1)
            assert= true $ empty? $ #{}
            assert= false $ empty? $ #{} 1
            assert= false $ empty? ||
            assert= false $ empty? |hello
            ; Contains trait
            assert= true $ contains? ([] 1 2 3) 2
            assert= false $ contains? ([] 1 2 3) 4
            assert= true $ contains?
              {} $ :a 1
              , :a
            assert= false $ contains?
              {} $ :a 1
              , :b
            assert= true $ contains? (#{} 1 2 3) 2
            assert= false $ contains? (#{} 1 2 3) 4
            let
                xs $ [] 1 2 3
                m $ {} $ :a 1
                s $ #{} 1 2
                text |abc
                enum_value $ :: :demo 1
                struct_value $ %{} Person0 $ :name |A
              assert= 3 $ count-with-trait xs
              assert= 1 $ count-with-trait m
              assert= 2 $ count-with-trait s
              assert= 3 $ count-with-trait text
              assert= 2 $ count-with-trait enum_value
              assert= 1 $ count-with-trait struct_value
              assert= true $ contains-with-trait? enum_value 1
              assert= true $ contains-with-trait? struct_value :name
              assert-traits xs Countable
              ; Map and Set left the Contains trait in 0.29.0; the prefix "`contains?`" still accepts them.
              assert-traits m Countable
              assert-traits s Countable
              assert-traits text Countable
              assert-traits enum_value Countable Contains
              assert-traits struct_value Countable Contains
            println "|  Collection traits: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-compare-trait $ %{} 'CodeEntry (:doc "|Test Compare trait")
          :code $ quote $ defn test-compare-trait () (println "|Testing Compare trait...") (; Number comparison)
            assert= true $ < 1 2
            assert= true $ > 2 1
            assert= true $ <= 1 1
            assert= true $ >= 2 2
            ; String comparison $ lexicographic
            assert= -1 $ &compare |apple |banana
            assert= 1 $ &compare |zebra |apple
            ; composite values compare by their elements, never by display text
            assert= -1 $ &compare ([] 2) ([] 10)
            assert= 1 $ &compare ([] 10) ([] 2)
            assert= 0 $ &compare ([] 1 2) ([] 1 2)
            assert= -1 $ &compare ([]) ([] 0)
            assert= -1 $ &compare ([] 1) ([] 1 0)
            assert= -1 $ &compare ([] 1 2) ([] 1 3)
            assert= -1 $ &compare
              [] $ [] 2
              [] $ [] 10
            assert= -1 $ &compare (#{} 2 3) (#{} 2 3 4)
            assert= -1 $ &compare (#{} 2 3) (#{} 2 10)
            assert= 1 $ &compare (#{} 10 3) (#{} 2 4)
            assert= 0 $ &compare (#{} 3 2) (#{} 2 3)
            assert= -1 $ &compare
              {} $ :a 2
              {} $ :a 10
            assert= -1 $ &compare
              {} $ :a 1
              {} (:a 1) (:b 1)
            assert= 1 $ &compare
              {} $ :b 1
              {} $ :a 1
            assert= 0 $ &compare
              {} (:a 1) (:b 2)
              {} (:b 2) (:a 1)
            do
              assert= -1 $ .compare 1 2
              assert= 0 $ .compare 2 2
              assert= 1 $ .compare 3 2
              assert= -1 $ .compare |apple |banana
              assert= 1 $ .compare |zebra |apple
              assert= -1 $ compare-with-trait 1 2
              assert= -1 $ compare-with-trait |a |b
              assert-traits 1 Compare
              assert-traits |a Compare
            println "|  Compare trait: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-cross-trait-method-conflict $ %{} 'CodeEntry (:doc "|Test method conflict across traits")
          :code $ quote $ defn test-cross-trait-method-conflict () (println "|Testing cross-trait method conflict...")
            let
                ; two different traits provide the same method name "`:zap`"
                ; impl-traits appends impls, so later ones override earlier ones
                PersonA $ impl-traits Person0 MyZapAImpl MyZapBImpl
                PersonB $ impl-traits Person0 MyZapBImpl MyZapAImpl
                pa $ %{} PersonA $ :name |Alice
                pb $ %{} PersonB $ :name |Bob
                ta $ %:: DemoZapA :demo 1
                tb $ %:: DemoZapB :demo 1
              assert-traits pa MyZapA MyZapB
              assert-traits pb MyZapA MyZapB
              assert-traits ta MyZapA MyZapB
              assert-traits tb MyZapA MyZapB
              assert= |zapB $ &trait-call MyZapB :zap pa
              assert= |zapA $ &trait-call MyZapA :zap pb
              assert= |zapB $ &trait-call MyZapB :zap ta
              assert= |zapA $ &trait-call MyZapA :zap tb
            println "|  cross-trait conflict: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-debug-trait $ %{} 'CodeEntry (:doc "||Test Debug trait for built-in types")
          :code $ quote $ defn test-debug-trait () (println "|Testing Debug trait...") (; All built-in types should be debuggable)
            assert= |true $ str true
            assert= |false $ str false
            do
              assert= |42 $ str 42
              let
                  n 42
                assert= |42 $ n .debug
            assert= |hello $ str |hello
            assert= |:tag $ str :tag
            assert= "|([] 1 2 3)" $ str $ [] 1 2 3
            assert= "|({} (:a 1))" $ str $ {} (:a 1)
            ; assert= "|(#{} 1 2)" $ str $ #{} 1 2
            let
                Person $ impl-traits Person0 CoreShowImpl
                p $ %{} Person $ :name |Alice
              assert-traits p calcit.core/Show
              assert= |Person:Alice $ p .show
            println "|  Debug trait: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-deftrait $ %{} 'CodeEntry (:doc "|Test deftrait macro")
          :code $ quote $ defn test-deftrait () (println "|Testing deftrait macro...")
            assert= :trait $ type-of MyFoo
            let
                Person $ impl-traits Person0 MyFooImpl
                p $ %{} Person $ :name |Alice
              assert= "|foo Alice" $ p .foo
              println "|  deftrait: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |computed-method-metadata-evaluates-once)
              :code $ quote $ let
                  calls $ ref 0
                  signature $ fn () (swap! calls inc)
                    quote $ :: 'Fn $ {}
                      :args $ [] 'Number
                      :return 'Number
                  built $ &trait::new :ComputedSignature $ []
                    [] :value $ signature
                assert= :trait $ type-of built
                assert= 1 @calls
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |accepts-external-field-pair)
              :code $ quote $ assert= false
                throws-at-expansion? $ deftrait Host $ :icons 'Dynamic
            %{} 'TestEntry (:name |rejects-nested-field-pair)
              :code $ quote $ assert= true
                throws-at-expansion? $ deftrait Host $
                  :icons 'Dynamic
        'test-enum-impl-precedence-order $ %{} 'CodeEntry (:doc "|Test enum impl precedence order")
          :code $ quote $ defn test-enum-impl-precedence-order () (println "|Testing enum impl precedence order...")
            let
                t $ %:: DemoBar :demo 1
              assert-traits t MyBar
              assert= |bar2 $ t .bar
            println "|  tuple precedence: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-eq-trait $ %{} 'CodeEntry (:doc "|Test Eq trait")
          :code $ quote $ defn test-eq-trait () (println "|Testing Eq trait...") (; Value equality)
            assert= true $ = 1 1
            assert= true $ = |hello |hello
            assert= true $ = :tag :tag
            assert= true $ = ([] 1 2) ([] 1 2)
            assert= true $ =
              {} $ :a 1
              {} $ :a 1
            ; Inequality
            assert= false $ = 1 2
            assert= false $ = |hello |world
            assert= false $ = ([] 1 2) ([] 1 2 3)
            println "|  Eq trait: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-explicit-trait-call $ %{} 'CodeEntry (:doc "|Test explicit trait-call for disambiguation")
          :code $ quote $ defn test-explicit-trait-call () (println "|Testing explicit trait-call...")
            let
                Person $ impl-traits Person0 MyZapAImpl MyZapBImpl
                p $ %{} Person $ :name |Alice
              assert-traits p MyZapA MyZapB
              ; strict dispatch requires the nominal trait origin to be explicit
              assert= |zapB $ &trait-call MyZapB :zap p
              ; "`&trait-call`" selects by trait, bypassing "`.method`" ambiguity
              assert= |zapA $ &trait-call MyZapA :zap p
              assert= |zapB $ &trait-call MyZapB :zap p
            let
                SinglePerson $ impl-traits Person0 MyZapAImpl
                p $ %{} SinglePerson $ :name |Bob
              assert= |zapA $ &trait-call MyZapA :zap p
              assert= :true $ try
                do (&trait-call MyZapB :zap p) :false
                fn (e) (do :true)
            let
                xs $ [] 1 2 3
                flag true
              assert= 3 $ &trait-call calcit.core/Countable :count xs
              assert= |true $ &trait-call calcit.core/Debug :debug flag
              assert= true $ &trait-call calcit.core/Eq :eq? flag true
            let
                t $ %:: DemoZap :demo 1
              assert-traits t MyZapA MyZapB
              assert= |zapB $ &trait-call MyZapB :zap t
              assert= |zapA $ &trait-call MyZapA :zap t
              assert= |zapB $ &trait-call MyZapB :zap t
            println "|  explicit trait-call: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |count-with-proven-list)
              :code $ quote $ assert= 2
                &trait-call calcit.core/Countable :count $ [] 1 2
              :tags $ #{} :trait :unit
            %{} 'TestEntry (:name |preserves-declared-generic-bound)
              :code $ quote $ let
                  count-items $ fn (x)
                    hint-fn $ {} (:return 'Number)
                      :args $ [] 'T
                      :generics $ [] 'T
                      :where $ {} $ 'T 'Countable
                    &trait-call calcit.core/Countable :count x
                assert= 2 $ count-items $ [] 1 2
              :tags $ #{} :trait :unit
        'test-impl-precedence-order $ %{} 'CodeEntry (:doc "|Test impl precedence order")
          :code $ quote $ defn test-impl-precedence-order () (println "|Testing impl precedence order...")
            let
                ; strict mode keeps one implementation per nominal trait origin
                Person $ impl-traits Person0 MyFooImpl2
                p $ %{} Person $ :name |Alice
              assert= "|foo2 Alice" $ p .foo
            println "|  precedence: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-method-introspection $ %{} 'CodeEntry (:doc "|Test runtime method introspection helpers")
          :code $ quote $ defn test-method-introspection () (println "|Testing method introspection...")
            let
                xs $ [] 1 2
                ms $ &methods-of xs
              assert= :list $ type-of ms
              assert= true $ includes? ms .add
              assert= true $ includes? ms .count
              assert= true $ includes? ms .includes?
              ; "`&inspect-methods`" returns the original value unchanged
              assert= xs $ &inspect-methods xs |list
            let
                Person $ impl-traits Person0 MyFooImpl
                p $ %{} Person $ :name |Alice
                ms2 $ &methods-of p
              assert= true $ includes? ms2 .foo
              assert= p $ &inspect-methods p |struct
            let
                ms3 $ &methods-of $ impl-traits Person0 MyFooImpl
                ms4 $ &methods-of DemoBar
                ms5 $ &methods-of MyFoo
              assert= true $ includes? ms3 .foo
              assert= true $ includes? ms4 .bar
              assert= true $ includes? ms5 .foo
            println "|  method introspection: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-option-result-map $ %{} 'CodeEntry (:doc "|Test Mappable trait for Option/Result")
          :code $ quote $ defn test-option-result-map () (println "|Testing Option/Result Mappable...")
            let
                opt-some $ Option :some 1
                opt-none $ assert-type (Option :none) (:: 'Option 'Number)
                res-ok $ Result :ok 1
                res-err $ assert-type (Result :err |oops) (:: 'Result 'Number 'String)
                step $ fn (x)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  inc x
              assert-type opt-some $ :: 'Option 'Number
              assert-type opt-none $ :: 'Option 'Number
              assert-type res-ok $ :: 'Result 'Number 'String
              assert-type res-err $ :: 'Result 'Number 'String
              assert= (Option :some 2) (opt-some .map step)
              assert= (Option :none) (opt-none .map step)
              assert= (Result :ok 2) (res-ok .map step)
              assert= (Result :err |oops) (res-err .map step)
              assert= (Option :some 2)
                opt-some .map $ fn (x) (inc x)
            let
                opt-some $ Option :some 1
                opt-none $ assert-type (Option :none) (:: 'Option 'Number)
                res-ok $ Result :ok 1
                res-err $ assert-type (Result :err |oops) (:: 'Result 'Number 'String)
                to-some $ fn (x)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return $ :: 'Option 'Number
                  Option :some $ inc x
                to-ok $ fn (x)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return $ :: 'Result 'Number 'String
                  Result :ok $ inc x
              assert= true $ opt-some .some?
              assert= true $ opt-none .none?
              assert= 1 $ opt-some .unwrap-or 9
              assert= 9 $ opt-none .unwrap-or 9
              assert= (Option :some 2) (opt-some .and-then to-some)
              assert= (Option :none) (opt-none .and-then to-some)
              assert= true $ res-ok .ok?
              assert= true $ res-err .err?
              assert= 1 $ res-ok .unwrap-or 9
              assert= 9 $ res-err .unwrap-or 9
              assert= 1 $
                res-ok .unwrap-or 9
                , .round
              assert= 9 $
                res-err .unwrap-or 9
                , .round
              assert= 9 $
                  assert-type (Option :none) (:: 'Option 'Number)
                  , .unwrap-or 9
                , .round
              assert= (Result :ok 2) (res-ok .and-then to-ok)
              assert= (Result :err |oops) (res-err .and-then to-ok)
              assert= (Result :ok 1) (res-ok .map-err turn-tag)
              assert= (Result :err :oops) (res-err .map-err turn-tag)
            let
                open-map $ assert-type
                  {} (:a 1) (:b |x)
                  :: 'Map 'Tag 'Dynamic
                open-list $ assert-type ([] 1 |x) (:: 'List 'Dynamic)
                present $ get open-map :a
              do
                assert= (Option :some 1) present
                assert= (Option :some |x) (nth open-list 1)
                assert= 1 $ option:unwrap-or present 0
                assert= 1 $ option:fold present
                  fn () 0
                  fn (value) value
                assert= 1 $ if-let (value present) value 0
                assert= true $ =
                  option:unwrap-or (first open-list) 0
                  , 1
                assert= true $ = 1 $ option:unwrap-or (first open-list) 0
                assert=
                  [] ([] 0 1) ([] 1 |x)
                  map-indexed open-list $ fn (idx value) ([] idx value)
            println "|  Option/Result map: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-qualified-contains-boundary $ %{} 'CodeEntry (:doc "|验证同名 trait 方法在显式调用与 Contains 泛型约束下保持各自来源。")
          :code $ quote $ defn test-qualified-contains-boundary ()
            let
                CustomContains $ deftrait CustomContains $ .contains? :fn
                CustomContainsImpl $ defimpl CustomContainsImpl CustomContains $ .contains?
                  fn (self field) (&= field :virtual)
                Box $ impl-traits
                  defstruct Box $ :value 'Tag
                  , CustomContainsImpl
                box $ %{} Box $ :value :x
              and (&trait-call CustomContains :contains? box :virtual)
                not $ &trait-call CustomContains :contains? box :value
                &trait-call Contains :contains? box :value
                not $ &trait-call Contains :contains? box :virtual
                contains-with-trait? box :value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |dispatches-by-bound-trait)
            :code $ quote $ assert= true (test-qualified-contains-boundary)
            :tags $ #{} :predicate :trait :unit
        'test-to-string-custom $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-to-string-custom ()
            let
                Person $ impl-traits Person0 CoreToStringImpl
                p $ %{} Person $ :name |Alice
              assert= |Person:Alice $ turn-string p
              to-string p
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |dispatches-custom-to-string)
            :code $ quote $ assert= |Person:Alice (test-to-string-custom)
            :tags $ #{} :trait :unit
        'test-trait-requires $ %{} 'CodeEntry (:doc "|Test requires between ordinary traits")
          :code $ quote $ defn test-trait-requires ()
            let
                g $ Greeter :name |Ada
              assert= "|hello Ada / Ada" $ describe-greeting g
              assert= g $ assert-traits g Greeting
              assert= g $ assert-traits g Labeled
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'throws-at-expansion? $ %{} 'CodeEntry
          :doc "|Evaluate macro rejection during expansion and emit only its boolean result for shared backend replay."
          :code $ quote $ defmacro throws-at-expansion? (form)
            let
                failed? $ throws? $ macroexpand form
              quasiquote ~failed?
          :examples $ []
          :schema $ :: 'Macro $ {}
            :capabilities $ #{}
            :expansion $ :: 'Expr 'Bool
            :required $ [] 'Syntax
        'trait-listener-host $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn trait-listener-host (value)
            unsafe-coerce value $ :: 'Fn $ {}
              :args $ [] 'test-traits.main/MyFoo
              :return 'Unit
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
            :return $ :: 'Fn $ {} (:return 'Unit)
              :args $ [] 'test-traits.main/MyFoo
          :tests $ [] $ %{} 'TestEntry (:name |nominal-trait-callback)
            :code $ quote $ assert= :fn
              type-of $ trait-listener-host $ fn (value) &unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.main
          :require $ calcit.test :refer $ throws?
    'test-traits.struct-identity $ %{} 'FileEntry
      :defs $ {} $ 'comparison-contract?
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn comparison-contract? () true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |preserves-equivalent-definition-control)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value 'Number
                  right $ defstruct ComparedRecord $ :value 'Number
                assert= true $ = left right
                assert= 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |distinguishes-field-contracts)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value 'Number
                  right $ defstruct ComparedRecord $ :value 'String
                assert= false $ = left right
                assert= 2 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |distinguishes-declared-generic-slots)
              :code $ quote $ let
                  left $ defstruct ComparedRecord ([] 'T) (:value 'Number)
                  right $ defstruct ComparedRecord ([] 'T 'U) (:value 'Number)
                assert= false $ = left right
                assert= 2 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |distinguishes-trait-bounds)
              :code $ quote $ let
                  left $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T Debug
                    :value 'T
                  right $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T ToString
                    :value 'T
                assert= false $ = left right
                assert= 2 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-canonical-field-type-aliases)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value 'Number
                  right $ defstruct ComparedRecord $ :value :number
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |orders-same-named-nominal-bounds)
              :code $ quote $ let
                  left-origin $ deftrait Marker
                  right-origin $ deftrait Marker
                  left $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T left-origin
                    :value 'T
                  right $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T right-origin
                    :value 'T
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |distinguishes-old-edn-definitions)
              :code $ quote $ let
                  Point $ defstruct Point $ :x 'Number
                  live $ Point :x 1
                  parsed $ parse-cirru-edn $ format-cirru-edn live
                if (struct? parsed)
                  assert= false $ =
                    .unwrap $ struct-definition live
                    .unwrap $ struct-definition parsed
                  raise |expected-struct
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-generics-through-impl-decoration)
              :code $ quote $ let
                  marker $ deftrait Marker
                  implementation $ defimpl MarkerImpl marker
                  left $ defstruct ComparedRecord ([] 'T) (:value 'Number)
                  right $ defstruct ComparedRecord ([] 'T 'U) (:value 'Number)
                assert= false $ = (impl-traits left implementation) (impl-traits right implementation)
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-equivalent-trait-bound-forms)
              :code $ quote $ let
                  marker $ deftrait Marker
                  left $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T marker
                    :value 'T
                  right $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T $ [] marker
                    :value 'T
                assert= true $ = left right
                assert= 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-nested-container-aliases)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value
                    :: 'Map 'Tag $ :: 'List 'Number
                  right $ defstruct ComparedRecord $ :value
                    :: :map :tag $ :: :list :number
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-default-container-arguments)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value 'List
                  right $ defstruct ComparedRecord $ :value (:: 'List 'Dynamic)
                assert= true $ = left right
                assert= 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-function-type-aliases)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :return 'String
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] :number
                      :return :string
                assert= true $ = left right
                assert= 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |orders-distinct-function-rest-contracts)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :rest 'String
                      :return 'Bool
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :rest 'Number
                      :return 'Bool
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |orders-distinct-function-feature-contracts)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :return 'Bool
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :return 'Bool
                      :features $ #{} :js-ffi
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |orders-nominal-trait-field-contracts)
              :code $ quote $ let
                  left-origin $ deftrait Marker
                  right-origin $ deftrait Marker
                  left $ defstruct ComparedRecord $ :value left-origin
                  right $ defstruct ComparedRecord $ :value right-origin
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |distinguishes-local-field-nominal-origins)
              :code $ quote $ let
                  left $ test-traits.struct-left/make-holder
                  right $ test-traits.struct-right/make-holder
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |distinguishes-source-nominal-origins)
              :code $ quote $ let
                  left $ test-traits.struct-left/definition
                  right $ test-traits.struct-right/definition
                  values $ {} (left |left) (right |right)
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
                assert= 2 $ count $ #{} left right
                assert= (Option :some |left) (.get values left)
                assert= (Option :some |right) (.get values right)
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-generic-shadowing-of-nominal-names)
              :code $ quote $ let
                  left $ test-traits.struct-left/make-generic-holder
                  right $ test-traits.struct-right/make-generic-holder
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-function-schema-defaults)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {} $ :return 'Bool
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ []
                      :return 'Bool
                      :kind :fn
                      :features $ #{}
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-serialized-open-field-contracts)
              :code $ quote $ let
                  Open $ defstruct Open $ :value 'Dynamic
                  live $ Open :value 1
                  parsed $ parse-cirru-edn $ format-cirru-edn live
                if (struct? parsed)
                  assert= true $ =
                    .unwrap $ struct-definition live
                    .unwrap $ struct-definition parsed
                  raise |expected-struct
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-deduplicated-generic-parameters)
              :code $ quote $ let
                  left $ defstruct ComparedRecord ([] 'T) (:value 'T)
                  right $ defstruct ComparedRecord ([] 'T 'T) (:value 'T)
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-function-generic-defaults)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :return 'Bool
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ [] 'Number
                      :return 'Bool
                      :generics $ []
                      :where $ {}
                      :async false
                assert= true $ = left right
                assert= 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |orders-function-async-contracts)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ []
                      :return 'Bool
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                      :args $ []
                      :return 'Bool
                      :async true
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-open-callback-spellings)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :callback 'Fn
                  right $ defstruct ComparedRecord $ :callback
                    :: 'Fn $ {}
                  applied $ defstruct ComparedRecord $ :callback (:: 'Fn)
                assert= true $ = left right
                assert= true $ = left applied
                assert= 0 $ &compare left right
                assert= 0 $ &compare left applied
                assert= 1 $ count $ #{} left right applied
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-host-nullable-type-aliases)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value (:: 'JsNullish 'Number)
                  right $ defstruct ComparedRecord $ :value (:: :js-nullish :number)
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-variadic-type-aliases)
              :code $ quote $ let
                  left $ defstruct ComparedRecord $ :value (:: 'Variadic 'Number)
                  right $ defstruct ComparedRecord $ :value (:: :variadic :number)
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |does-not-invent-host-type-aliases)
              :code $ quote $ let
                  named $ test-traits.struct-left/make-host-holder
                  host $ defstruct ComparedRecord $ :value 'JsObject
                assert= false $ = named host
                assert= false $ = 0 $ &compare named host
                assert= 2 $ count $ #{} named host
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-string-bound-keys)
              :code $ quote $ let
                  marker $ deftrait Marker
                  left $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T marker
                    :value 'T
                  right $ defstruct ComparedRecord ([] 'T)
                    {} $ |:T marker
                    :value 'T
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |does-not-invent-bounds-from-tag-keys)
              :code $ quote $ let
                  marker $ deftrait Marker
                  left $ defstruct ComparedRecord ([] 'T) (:value 'T)
                  right $ defstruct ComparedRecord ([] 'T)
                    {} $ :T marker
                    :value 'T
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |does-not-invent-bounds-from-scalar-types)
              :code $ quote $ let
                  left $ defstruct ComparedRecord ([] 'T) (:value 'T)
                  right $ defstruct ComparedRecord ([] 'T)
                    {} $ 'T 'Number
                    :value 'T
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry (:name |preserves-native-builtin-type-priority)
              :code $ quote $ let
                  left $ test-traits.struct-left/make-builtin-holder
                  right $ test-traits.struct-right/make-builtin-holder
                assert= true $ = left right
                assert= 0 $ &compare left right
                assert= 1 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry
              :name |distinguishes-qualified-builtin-spelled-nominal-types
              :code $ quote $ let
                  left $ test-traits.struct-left/make-qualified-holder
                  right $ test-traits.struct-right/make-qualified-holder
                assert= false $ = left right
                assert= false $ = 0 $ &compare left right
                assert= 2 $ count $ #{} left right
              :tags $ #{} :struct-definition-equality :unit
            %{} 'TestEntry
              :name |distinguishes-builtin-marker-from-qualified-nominal-type
              :code $ quote $ let
                  builtin $ test-traits.struct-left/make-builtin-holder
                  nominal $ test-traits.struct-left/make-qualified-holder
                assert= false $ = builtin nominal
                assert= false $ = 0 $ &compare builtin nominal
                assert= 2 $ count $ #{} builtin nominal
              :tags $ #{} :struct-definition-equality :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.struct-identity
    'test-traits.struct-left $ %{} 'FileEntry
      :defs $ {}
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'StructDef $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct StructDef (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'definition $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn definition () Point
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'jsobject $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct jsobject (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'make-builtin-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-builtin-holder ()
            defstruct Holder $ :value 'StructDef
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-generic-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-generic-holder ()
            defstruct Holder ([] 'Point) (:point 'Point)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-holder ()
            defstruct Holder $ :point 'Point
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-host-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-host-holder ()
            defstruct ComparedRecord $ :value 'jsobject
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-qualified-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-qualified-holder ()
            defstruct Holder $ :value 'test-traits.struct-left/StructDef
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.struct-left
    'test-traits.struct-right $ %{} 'FileEntry
      :defs $ {}
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'StructDef $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct StructDef (:value 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'definition $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn definition () Point
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-builtin-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-builtin-holder ()
            defstruct Holder $ :value 'StructDef
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-generic-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-generic-holder ()
            defstruct Holder ([] 'Point) (:point 'Point)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-holder ()
            defstruct Holder $ :point 'Point
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
        'make-qualified-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-qualified-holder ()
            defstruct Holder $ :value 'test-traits.struct-right/StructDef
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'StructDef)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.struct-right
