
{} (:package |test-traits)
  :configs $ {} (:init-fn |test-traits.main/main!) (:reload-fn |test-traits.main/main!)
  :files $ {}
    |test-traits.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              &init-builtin-impls!
              println "|Testing built-in traits..."

              ; Test Show trait - all types should have it
              test-show-trait

              ; Test deftrait macro
              test-deftrait

              ; Test impl precedence order
              test-impl-precedence-order
              test-tuple-impl-precedence-order
              test-cross-trait-method-conflict
              test-explicit-trait-call

              ; Test Eq trait
              test-eq-trait

              ; Test Compare trait
              test-compare-trait

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

              println "|All trait tests passed!"
          :examples $ []

        |test-show-trait $ %{} :CodeEntry (:doc "|Test Show trait for built-in types")
          :code $ quote
            defn test-show-trait ()
              println "|Testing Show trait..."

              ; All types should be showable
              assert= "|true" $ str true
              assert= "|false" $ str false
              assert= "|42" $ str 42
              assert= "|hello" $ str |hello
              assert= "|:tag" $ str :tag
              assert= "|([] 1 2 3)" $ str ([] 1 2 3)
              assert= "|({} (:a 1))" $ str ({} (:a 1))
              ; assert= "|(#{} 1 2)" $ str (#{} 1 2)

              println "|  Show trait: ✓"
          :examples $ []

        |MyFoo $ %{} :CodeEntry (:doc "|Trait for deftrait test")
          :code $ quote
            deftrait MyFoo
              :foo :fn
          :examples $ []

        |MyFooImpl $ %{} :CodeEntry (:doc "|Trait impl for deftrait test")
          :code $ quote
            defimpl MyFooImpl MyFoo
              :foo $ fn (p) (str "|foo " (:name p))
          :examples $ []

        |Person0 $ %{} :CodeEntry (:doc "|Struct used in trait tests")
          :code $ quote
            defstruct Person0
              :name :string
          :examples $ []

        |MyFooImpl2 $ %{} :CodeEntry (:doc "|Trait impl for override test")
          :code $ quote
            defimpl MyFooImpl2 MyFoo
              :foo $ fn (p) (str "|foo2 " (:name p))
          :examples $ []

        |MyBar $ %{} :CodeEntry (:doc "|Trait for tuple override test")
          :code $ quote
            deftrait MyBar
              :bar :fn
          :examples $ []

        |MyBarImpl $ %{} :CodeEntry (:doc "|Trait impl for tuple override test")
          :code $ quote
            defimpl MyBarImpl MyBar
              :bar $ fn (_x) "|bar1"
          :examples $ []

        |MyBarImpl2 $ %{} :CodeEntry (:doc "|Trait impl for tuple override test")
          :code $ quote
            defimpl MyBarImpl2 MyBar
              :bar $ fn (_x) "|bar2"
          :examples $ []

        |MyZapA $ %{} :CodeEntry (:doc "|Trait A for cross-trait method conflict test")
          :code $ quote
            deftrait MyZapA
              :zap :fn
          :examples $ []

        |MyZapB $ %{} :CodeEntry (:doc "|Trait B for cross-trait method conflict test")
          :code $ quote
            deftrait MyZapB
              :zap :fn
          :examples $ []

        |MyZapAImpl $ %{} :CodeEntry (:doc "|Trait A impl for cross-trait method conflict test")
          :code $ quote
            defimpl MyZapAImpl MyZapA
              :zap $ fn (_x) "|zapA"
          :examples $ []

        |MyZapBImpl $ %{} :CodeEntry (:doc "|Trait B impl for cross-trait method conflict test")
          :code $ quote
            defimpl MyZapBImpl MyZapB
              :zap $ fn (_x) "|zapB"
          :examples $ []

        |Demo0 $ %{} :CodeEntry (:doc "|Enum prototype for tuple trait tests")
          :code $ quote
            defenum Demo
              :demo :dynamic
          :examples $ []

        |DemoBar $ %{} :CodeEntry (:doc "|Enum with MyBar impls")
          :code $ quote
            def DemoBar $ impl-traits Demo0 MyBarImpl MyBarImpl2
          :examples $ []

        |DemoZapA $ %{} :CodeEntry (:doc "|Enum with MyZapA then MyZapB")
          :code $ quote
            def DemoZapA $ impl-traits Demo0 MyZapAImpl MyZapBImpl
          :examples $ []

        |DemoZapB $ %{} :CodeEntry (:doc "|Enum with MyZapB then MyZapA")
          :code $ quote
            def DemoZapB $ impl-traits Demo0 MyZapBImpl MyZapAImpl
          :examples $ []

        |DemoZap $ %{} :CodeEntry (:doc "|Enum with MyZapA/MyZapB")
          :code $ quote
            def DemoZap $ impl-traits Demo0 MyZapAImpl MyZapBImpl
          :examples $ []

        |test-deftrait $ %{} :CodeEntry (:doc "|Test deftrait macro")
          :code $ quote
            defn test-deftrait ()
              println "|Testing deftrait macro..."
              assert= :trait $ type-of MyFoo
              let
                  Person $ impl-traits Person0 MyFooImpl
                  p $ %{} Person (:name |Alice)
                assert= "|foo Alice" $ .foo p
                println "|  deftrait: ✓"
          :examples $ []

        |test-impl-precedence-order $ %{} :CodeEntry (:doc "|Test impl precedence order")
          :code $ quote
            defn test-impl-precedence-order ()
              println "|Testing impl precedence order..."
              let
                  ; impl-traits appends impls, so later ones override earlier ones
                  Person $ impl-traits Person0 MyFooImpl MyFooImpl2
                  p $ %{} Person (:name |Alice)
                assert= "|foo2 Alice" $ .foo p
              println "|  precedence: ✓"
          :examples $ []

        |test-tuple-impl-precedence-order $ %{} :CodeEntry (:doc "|Test tuple impl precedence order")
          :code $ quote
            defn test-tuple-impl-precedence-order ()
              println "|Testing tuple impl precedence order..."
              let
                  t $ %:: DemoBar :demo 1
                assert-traits t MyBar
                assert= "|bar2" $ .bar t
              println "|  tuple precedence: ✓"
          :examples $ []

        |test-cross-trait-method-conflict $ %{} :CodeEntry (:doc "|Test method conflict across traits")
          :code $ quote
            defn test-cross-trait-method-conflict ()
              println "|Testing cross-trait method conflict..."
              let
                  ; two different traits provide the same method name `:zap`
                  ; impl-traits appends impls, so later ones override earlier ones
                  PersonA $ impl-traits Person0 MyZapAImpl MyZapBImpl
                  PersonB $ impl-traits Person0 MyZapBImpl MyZapAImpl
                  pa $ %{} PersonA (:name |Alice)
                  pb $ %{} PersonB (:name |Bob)

                  ta $ %:: DemoZapA :demo 1
                  tb $ %:: DemoZapB :demo 1
                assert-traits pa MyZapA MyZapB
                assert-traits pb MyZapA MyZapB
                assert-traits ta MyZapA MyZapB
                assert-traits tb MyZapA MyZapB
                assert= "|zapB" $ .zap pa
                assert= "|zapA" $ .zap pb
                assert= "|zapB" $ .zap ta
                assert= "|zapA" $ .zap tb
              println "|  cross-trait conflict: ✓"
          :examples $ []

        |test-explicit-trait-call $ %{} :CodeEntry (:doc "|Test explicit trait-call for disambiguation")
          :code $ quote
            defn test-explicit-trait-call ()
              println "|Testing explicit trait-call..."
              let
                  Person $ impl-traits Person0 MyZapAImpl MyZapBImpl
                  p $ %{} Person (:name |Alice)
                assert-traits p MyZapA MyZapB
                ; `.zap` follows normal dispatch (last-wins for user impls)
                assert= "|zapB" $ .zap p
                ; `&trait-call` selects by trait, bypassing `.method` ambiguity
                assert= "|zapA" $ &trait-call MyZapA :zap p
                assert= "|zapB" $ &trait-call MyZapB :zap p

              let
                  t $ %:: DemoZap :demo 1
                assert-traits t MyZapA MyZapB
                assert= "|zapB" $ .zap t
                assert= "|zapA" $ &trait-call MyZapA :zap t
                assert= "|zapB" $ &trait-call MyZapB :zap t

              println "|  explicit trait-call: ✓"
          :examples $ []

        |test-eq-trait $ %{} :CodeEntry (:doc "|Test Eq trait")
          :code $ quote
            defn test-eq-trait ()
              println "|Testing Eq trait..."

              ; Value equality
              assert= true $ = 1 1
              assert= true $ = |hello |hello
              assert= true $ = :tag :tag
              assert= true $ = ([] 1 2) ([] 1 2)
              assert= true $ = ({} (:a 1)) ({} (:a 1))

              ; Inequality
              assert= false $ = 1 2
              assert= false $ = |hello |world
              assert= false $ = ([] 1 2) ([] 1 2 3)

              println "|  Eq trait: ✓"
          :examples $ []

        |test-compare-trait $ %{} :CodeEntry (:doc "|Test Compare trait")
          :code $ quote
            defn test-compare-trait ()
              println "|Testing Compare trait..."

              ; Number comparison
              assert= true $ < 1 2
              assert= true $ > 2 1
              assert= true $ <= 1 1
              assert= true $ >= 2 2

              ; String comparison (lexicographic)
              assert= -1 $ &compare |apple |banana
              assert= 1 $ &compare |zebra |apple

              ; List comparison (not yet implemented in compare form)
              ; assert= :lt $ compare ([] 1 2) ([] 1 3)

              println "|  Compare trait: ✓"
          :examples $ []

        |test-add-trait $ %{} :CodeEntry (:doc "|Test Add trait")
          :code $ quote
            defn test-add-trait ()
              println "|Testing Add trait..."

              ; Number addition
              assert= 3 $ + 1 2
              assert= 10 $ + 1 2 3 4

              ; String concatenation (using str)
              assert= "|hello world" $ str-spaced |hello |world

              ; List concatenation
              assert= ([] 1 2 3 4) $ &list:concat ([] 1 2) ([] 3 4)

              ; Regression: list `.add` should keep list-method semantics.
              ; It must not be shadowed by Add trait `:add` in `&core-list-impls`.
              assert= ([] 1 2)
                .add ([] 1) 2

              println "|  Add trait: ✓"
          :examples $ []

        |test-collection-traits $ %{} :CodeEntry (:doc "|Test Len/Empty/Contains traits for collections")
          :code $ quote
            defn test-collection-traits ()
              println "|Testing Collection traits (Len, Empty)..."

              ; Len trait
              assert= 0 $ count ([])
              assert= 3 $ count ([] 1 2 3)
              assert= 5 $ count |hello
              assert= 2 $ count ({} (:a 1) (:b 2))
              assert= 3 $ count (#{} 1 2 3)

              ; Empty trait
              assert= true $ empty? ([])
              assert= false $ empty? ([] 1)
              assert= true $ empty? ({})
              assert= false $ empty? ({} (:a 1))
              assert= true $ empty? (#{})
              assert= false $ empty? (#{} 1)
              assert= false $ empty? ||
              assert= false $ empty? |hello

              ; Contains trait
              assert= true $ contains? ([] 1 2 3) 2
              assert= false $ contains? ([] 1 2 3) 4
              assert= true $ contains? ({} (:a 1)) :a
              assert= false $ contains? ({} (:a 1)) :b
              assert= true $ contains? (#{} 1 2 3) 2
              assert= false $ contains? (#{} 1 2 3) 4

              println "|  Collection traits: ✓"
          :examples $ []

        |test-option-result-map $ %{} :CodeEntry (:doc "|Test Mappable trait for Option/Result")
          :code $ quote
            defn test-option-result-map ()
              println "|Testing Option/Result Mappable..."

              let
                  opt-some $ %some 1
                  opt-none $ %none
                  res-ok $ %ok 1
                  res-err $ %err |oops
                assert-traits opt-some calcit.core/Mappable
                assert-traits opt-none calcit.core/Mappable
                assert-traits res-ok calcit.core/Mappable
                assert-traits res-err calcit.core/Mappable
                assert=
                  %some 2
                  .map opt-some inc
                assert=
                  %none
                  .map opt-none inc
                assert=
                  %ok 2
                  .map res-ok inc
                assert=
                  %err |oops
                  .map res-err inc

              println "|  Option/Result map: ✓"
          :examples $ []

        |test-assert-trait $ %{} :CodeEntry (:doc "|Test assert-traits")
          :code $ quote
            defn test-assert-trait ()
              println "|Testing assert-traits..."

              let
                  x 1
                  xs $ [] 1 2 3
                  m $ {} (:a 1) (:b 2)
                  s |hello
                  opt $ %some 1
                  Person $ impl-traits Person0 MyFooImpl
                  p $ %{} Person (:name |Alice)
                assert= x $ assert-traits x calcit.core/Show
                assert= x $ assert-traits x calcit.core/Show calcit.core/Eq
                assert= xs $ assert-traits xs calcit.core/Mappable
                assert= xs $ assert-traits xs calcit.core/Mappable calcit.core/Show
                assert= m $ assert-traits m calcit.core/Mappable
                assert= m $ assert-traits m calcit.core/Mappable calcit.core/Show
                assert= s $ assert-traits s calcit.core/Show
                assert= s $ assert-traits s calcit.core/Show calcit.core/Eq
                assert= opt $ assert-traits opt calcit.core/Mappable
                ; "Option only implements Mappable in current impls"
                assert= p $ assert-traits p MyFoo
                ; "MyFooImpl only provides :foo, no Show impl"

                assert= :true $ try
                  do (assert-traits p calcit.core/Show) :false
                  fn (e)
                    do
                      , :true

              println "|  assert-traits: ✓"
          :examples $ []

        |test-method-introspection $ %{} :CodeEntry (:doc "|Test runtime method introspection helpers")
          :code $ quote
            defn test-method-introspection ()
              println "|Testing method introspection..."
              let
                  xs $ [] 1 2
                  ms $ &methods-of xs
                assert= :list $ type-of ms
                assert= true $ includes? ms "|.add"
                assert= true $ includes? ms "|.count"
                assert= true $ includes? ms "|.includes?"

                ; `&inspect-methods` returns the original value unchanged
                assert= xs $ &inspect-methods xs "|list"

              let
                  Person $ impl-traits Person0 MyFooImpl
                  p $ %{} Person (:name |Alice)
                  ms2 $ &methods-of p
                assert= true $ includes? ms2 "|.foo"
                assert= p $ &inspect-methods p "|record"

              println "|  method introspection: ✓"
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-traits.main $ :require
        :examples $ []
