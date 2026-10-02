
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
            ; Built-in list .add appends one item to the receiver.
            ; The receiver type selects this method before the Add trait.
            assert= ([] 1 2)
              .add ([] 1) 2
            ; Legacy Map .add takes one pair, not the Add trait contract.
            assert=
              {} (:a 1) (:b 2)
              ({} (:a 1))
                , .add $ [] :b 2
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
                opt $ %some 1
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
              assert= true $ contains-with-trait? xs 1
              assert= true $ contains-with-trait? m :a
              assert= true $ contains-with-trait? s 2
              assert= true $ contains-with-trait? text 1
              assert= true $ contains-with-trait? enum_value 1
              assert= true $ contains-with-trait? struct_value :name
              assert-traits xs Countable Contains
              assert-traits m Countable Contains
              assert-traits s Countable Contains
              assert-traits text Countable Contains
              assert-traits enum_value Countable Contains
              assert-traits struct_value Countable Contains
            println "|  Collection traits: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-bit-shifts $ %{} 'CodeEntry (:doc "|Shift counts use their low five bits on every backend")
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
            assert= -1 $ &compare ([] ([] 2)) ([] ([] 10))
            assert= -1 $ &compare (#{} 2 3) (#{} 2 3 4)
            assert= -1 $ &compare (#{} 2 3) (#{} 2 10)
            assert= 1 $ &compare (#{} 10 3) (#{} 2 4)
            assert= 0 $ &compare (#{} 3 2) (#{} 2 3)
            assert= -1 $ &compare ({} (:a 2)) ({} (:a 10))
            assert= -1 $ &compare ({} (:a 1)) ({} (:a 1) (:b 1))
            assert= 1 $ &compare ({} (:b 1)) ({} (:a 1))
            assert= 0 $ &compare ({} (:a 1) (:b 2)) ({} (:b 2) (:a 1))
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
            %{} 'TestEntry (:name |accepts-external-field-pair)
              :code $ quote $ assert= false
                throws? $ macroexpand $ quote
                  deftrait Host $ :icons 'Dynamic
            %{} 'TestEntry (:name |rejects-nested-field-pair)
              :code $ quote $ assert= true
                throws? $ macroexpand $ quote
                  deftrait Host $
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
            %{} 'TestEntry (:name |duplicate-origin-runtime)
              :code $ quote $ let
                  Person $ impl-traits Person0 MyFooImpl MyFooImpl2
                  p $ %{} Person $ :name |Alice
                assert= true $ try
                  do (&trait-call MyFoo :foo p) false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error |duplicate
              :tags $ #{} :trait-runtime
            %{} 'TestEntry (:name |duplicate-origin-reversed)
              :code $ quote $ let
                  Person $ impl-traits Person0 MyFooImpl2 MyFooImpl
                  p $ %{} Person $ :name |Alice
                assert= true $ try
                  do (&trait-call MyFoo :foo p) false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error |duplicate
              :tags $ #{} :trait-runtime
            %{} 'TestEntry (:name |duplicate-origin-repeated)
              :code $ quote $ let
                  Person $ impl-traits Person0 MyFooImpl MyFooImpl
                  p $ %{} Person $ :name |Alice
                assert= true $ try
                  do (&trait-call MyFoo :foo p) false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error |duplicate
              :tags $ #{} :trait-runtime
            %{} 'TestEntry (:name |duplicate-enum-origin)
              :code $ quote $ let
                  Person $ impl-traits Demo0 MyBarImpl MyBarImpl2
                  p $ %:: Person :demo 1
                assert= true $ try
                  do (&trait-call MyBar :bar p) false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error |duplicate
              :tags $ #{} :trait-runtime
            %{} 'TestEntry (:name |duplicate-enum-reversed)
              :code $ quote $ let
                  Person $ impl-traits Demo0 MyBarImpl2 MyBarImpl
                  p $ %:: Person :demo 1
                assert= true $ try
                  do (&trait-call MyBar :bar p) false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error |duplicate
              :tags $ #{} :trait-runtime
            %{} 'TestEntry (:name |duplicate-arguments-once)
              :code $ quote $ let
                  Person $ impl-traits Person0 MyFooImpl MyFooImpl2
                  p $ %{} Person $ :name |Alice
                  counter $ atom 0
                assert= true $ try
                  do
                    &trait-call MyFoo :foo p $ reset! counter $ + @counter 1
                    , false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error |duplicate
                assert= 1 @counter
              :tags $ #{} :trait-runtime
            %{} 'TestEntry (:name |fresh-origin-rejects-stale-impl)
              :code $ quote $ let
                  make-trait $ fn ()
                    hint-fn $ {}
                      :args $ []
                      :return 'Trait
                    deftrait FreshShow $ .render :fn
                  old-trait $ make-trait
                  next-trait $ make-trait
                  old-impl $ defimpl OldImpl old-trait $ .render
                    fn (self) |old
                  Person $ impl-traits Person0 old-impl
                  p $ %{} Person $ :name |Alice
                assert= |old $ &trait-call old-trait :render p
                assert= true $ try
                  do (&trait-call next-trait :render p) false
                  fn (error)
                    hint-fn $ {}
                      :args $ [] 'String
                      :return 'Bool
                    &str:includes? error "|cannot find impl"
              :tags $ #{} :trait-identity-runtime
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
                opt-some $ %some 1
                opt-none $ assert-type (%none) (:: 'Option 'Number)
                res-ok $ %ok 1
                res-err $ assert-type (%err |oops) (:: 'Result 'Number 'String)
                step $ fn (x) (inc x)
              assert-type opt-some $ :: 'Option 'Number
              assert-type opt-none $ :: 'Option 'Number
              assert-type res-ok $ :: 'Result 'Number 'String
              assert-type res-err $ :: 'Result 'Number 'String
              assert= (%some 2) (opt-some .map step)
              assert= (%none) (opt-none .map step)
              assert= (%ok 2) (res-ok .map step)
              assert= (%err |oops) (res-err .map step)
              assert= (%some 2)
                opt-some .map $ fn (x) (inc x)
            let
                opt-some $ %some 1
                opt-none $ assert-type (%none) (:: 'Option 'Number)
                res-ok $ %ok 1
                res-err $ assert-type (%err |oops) (:: 'Result 'Number 'String)
                to-some $ fn (x)
                  %some $ inc x
                to-ok $ fn (x)
                  %ok $ inc x
              assert= true $ opt-some .some?
              assert= true $ opt-none .none?
              assert= 1 $ opt-some .unwrap-or 9
              assert= 9 $ opt-none .unwrap-or 9
              assert= (%some 2) (opt-some .and-then to-some)
              assert= (%none) (opt-none .and-then to-some)
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
                  assert-type (%none) (:: 'Option 'Number)
                  , .unwrap-or 9
                , .round
              assert= (%ok 2) (res-ok .and-then to-ok)
              assert= (%err |oops) (res-err .and-then to-ok)
              assert= (%ok 1) (res-ok .map-err turn-tag)
              assert= (%err :oops) (res-err .map-err turn-tag)
            let
                open-map $ assert-type
                  {} (:a 1) (:b |x)
                  :: 'Map 'Tag 'Dynamic
                open-list $ assert-type ([] 1 |x) (:: 'List 'Dynamic)
                present $ get open-map :a
              do
                assert= (%some 1) present
                assert= (%some |x) (nth open-list 1)
                assert= 1 $ option:unwrap-or present 0
                assert= 1 $ option:fold present
                  fn () 0
                  fn (value) value
                assert= 1 $ if-let (value present) value 0
                assert= true $ =
                  option:unwrap-or (first open-list) nil
                  , 1
                assert= true $ = 1 $ option:unwrap-or (first open-list) nil
                assert=
                  [] ([] 0 1) ([] 1 |x)
                  map-indexed open-list $ fn (idx value) ([] idx value)
            println "|  Option/Result map: ✓"
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
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
        'test-turn-str-custom $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-turn-str-custom ()
            let
                Person $ impl-traits Person0 CoreToStringImpl
                p $ %{} Person $ :name |Alice
              assert= |Person:Alice $ turn-string p
              turn-str p
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |dispatches-custom-to-string)
            :code $ quote $ assert= |Person:Alice (test-turn-str-custom)
            :tags $ #{} :trait :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-traits.main
          :require $ calcit.test :refer $ throws?
