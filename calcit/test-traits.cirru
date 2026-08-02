
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-traits) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-traits.main/main!) (:mode :native) (:reload-fn 'test-traits.main/main!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-traits.main $ %{} :FileEntry
      :defs $ {}
        |Demo0 $ %{} :CodeEntry (:doc "|Enum prototype for tuple trait tests")
          :code $ quote
            defenum Demo $ :demo 'Dynamic
          :examples $ []
          :schema $ :: 'Enum
        |DemoBar $ %{} :CodeEntry (:doc "|Enum with MyBar impls")
          :code $ quote
            def DemoBar $ impl-traits Demo0 MyBarImpl MyBarImpl2
          :examples $ []
          :schema $ :: 'Impl
        |DemoZap $ %{} :CodeEntry (:doc "|Enum with MyZapA/MyZapB")
          :code $ quote
            def DemoZap $ impl-traits Demo0 MyZapAImpl MyZapBImpl
          :examples $ []
          :schema $ :: 'Impl
        |DemoZapA $ %{} :CodeEntry (:doc "|Enum with MyZapA then MyZapB")
          :code $ quote
            def DemoZapA $ impl-traits Demo0 MyZapAImpl MyZapBImpl
          :examples $ []
          :schema $ :: 'Impl
        |DemoZapB $ %{} :CodeEntry (:doc "|Enum with MyZapB then MyZapA")
          :code $ quote
            def DemoZapB $ impl-traits Demo0 MyZapBImpl MyZapAImpl
          :examples $ []
          :schema $ :: 'Impl
        |MyBar $ %{} :CodeEntry (:doc "|Trait for tuple override test")
          :code $ quote
            deftrait MyBar $ .bar :fn
          :examples $ []
          :schema $ :: 'Trait
        |MyBarImpl $ %{} :CodeEntry (:doc "|Trait impl for tuple override test")
          :code $ quote
            defimpl MyBarImpl MyBar $ .bar mybar:bar1
          :examples $ []
          :schema $ :: 'Impl
        |MyBarImpl2 $ %{} :CodeEntry (:doc "|Trait impl for tuple override test")
          :code $ quote
            defimpl MyBarImpl2 MyBar $ .bar mybar:bar2
          :examples $ []
          :schema $ :: 'Impl
        |MyFoo $ %{} :CodeEntry (:doc "|Trait for deftrait test")
          :code $ quote
            deftrait MyFoo $ .foo :fn
          :examples $ []
          :schema $ :: 'Trait
        |MyFooImpl $ %{} :CodeEntry (:doc "|Trait impl for deftrait test")
          :code $ quote
            defimpl MyFooImpl MyFoo $ .foo myfoo:foo
          :examples $ []
          :schema $ :: 'Impl
        |MyFooImpl2 $ %{} :CodeEntry (:doc "|Trait impl for override test")
          :code $ quote
            defimpl MyFooImpl2 MyFoo $ .foo myfoo:foo2
          :examples $ []
          :schema $ :: 'Impl
        |MyZapA $ %{} :CodeEntry (:doc "|Trait A for cross-trait method conflict test")
          :code $ quote
            deftrait MyZapA $ .zap :fn
          :examples $ []
          :schema $ :: 'Trait
        |MyZapAImpl $ %{} :CodeEntry (:doc "|Trait A impl for cross-trait method conflict test")
          :code $ quote
            defimpl MyZapAImpl MyZapA $ .zap myzap:a
          :examples $ []
          :schema $ :: 'Impl
        |MyZapB $ %{} :CodeEntry (:doc "|Trait B for cross-trait method conflict test")
          :code $ quote
            deftrait MyZapB $ .zap :fn
          :examples $ []
          :schema $ :: 'Trait
        |MyZapBImpl $ %{} :CodeEntry (:doc "|Trait B impl for cross-trait method conflict test")
          :code $ quote
            defimpl MyZapBImpl MyZapB $ .zap myzap:b
          :examples $ []
          :schema $ :: 'Impl
        |Person0 $ %{} :CodeEntry (:doc "|Struct used in trait tests")
          :code $ quote
            defstruct Person0 $ :name 'String
          :examples $ []
          :schema $ :: 'Struct
        |compare-with-trait $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn compare-with-trait (a b) (.compare a b)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'T 'T
              :generics $ [] 'T
              :where $ {} ('T 'Compare)
        |contains-with-trait? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn contains-with-trait? (x k) (.contains? x k)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T 'K
              :generics $ [] 'T 'K
              :where $ {} ('T 'Contains)
        |count-with-trait $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn count-with-trait (x) (.count x)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'T
              :generics $ [] 'T
              :where $ {} ('T 'Countable)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (&init-builtin-impls!) (println "|Testing built-in traits...") (; Test Show trait - all types should have it) (test-show-trait) (; Test deftrait macro) (test-deftrait) (; Test impl precedence order) (test-impl-precedence-order) (test-tuple-impl-precedence-order) (test-cross-trait-method-conflict) (test-explicit-trait-call) (; Test Eq trait) (test-eq-trait) (; Test Compare trait) (test-compare-trait) (; Test Add trait) (test-add-trait) (; Test Len/Empty traits) (test-collection-traits) (; Test Option/Result Mappable) (test-option-result-map) (; Test assert-traits) (test-assert-trait) (; Debug helpers: methods introspection) (test-method-introspection) (println "|All trait tests passed!")
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |mybar:bar1 $ %{} :CodeEntry (:doc "|method implementation for MyBarImpl/:bar")
          :code $ quote
            defn mybar:bar1 (_x) |bar1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |mybar:bar2 $ %{} :CodeEntry (:doc "|method implementation for MyBarImpl2/:bar")
          :code $ quote
            defn mybar:bar2 (_x) |bar2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |myfoo:foo $ %{} :CodeEntry (:doc "|method implementation for MyFoo/:foo")
          :code $ quote
            defn myfoo:foo (p)
              str "|foo " $ :name p
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |myfoo:foo2 $ %{} :CodeEntry (:doc "|method implementation for MyFooImpl2/:foo")
          :code $ quote
            defn myfoo:foo2 (p)
              str "|foo2 " $ :name p
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |myzap:a $ %{} :CodeEntry (:doc "|method implementation for MyZapA/:zap")
          :code $ quote
            defn myzap:a (_x) |zapA
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |myzap:b $ %{} :CodeEntry (:doc "|method implementation for MyZapB/:zap")
          :code $ quote
            defn myzap:b (_x) |zapB
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |test-add-trait $ %{} :CodeEntry (:doc "|Test Add trait")
          :code $ quote
            defn test-add-trait () (println "|Testing Add trait...") (; Number addition)
              assert= 3 $ + 1 2
              assert= 10 $ + 1 2 3 4
              ; String concatenation $ using str
              assert= "|hello world" $ str-spaced |hello |world
              ; List concatenation
              assert= ([] 1 2 3 4)
                &list:concat ([] 1 2) ([] 3 4)
              ; Regression: list "`.add`" should keep list-method semantics.
              ; It must not be shadowed by Add trait "`:add`" in "`&core-list-impls`."
              assert= ([] 1 2)
                .add ([] 1) 2
              println "|  Add trait: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-assert-trait $ %{} :CodeEntry (:doc "|Test assert-traits")
          :code $ quote
            defn test-assert-trait () (println "|Testing assert-traits...")
              let
                  x 1
                  xs $ [] 1 2 3
                  m $ {} (:a 1) (:b 2)
                  st $ #{} 1 2 3
                  s |hello
                  opt $ %some 1
                  Person $ impl-traits Person0 MyFooImpl
                  p $ %{} Person (:name |Alice)
                  ZapPerson $ impl-traits Person0 MyZapAImpl
                  zp $ %{} ZapPerson (:name |Bob)
                  flag true
                  keyword :demo
                  nothing nil
                assert= x $ assert-traits x calcit.core/Show
                assert= x $ assert-traits x calcit.core/Show calcit.core/Eq
                assert= xs $ assert-traits xs calcit.core/Mappable
                assert= xs $ assert-traits xs calcit.core/Mappable calcit.core/Show
                assert= m $ assert-traits m calcit.core/Mappable
                assert= m $ assert-traits m calcit.core/Mappable calcit.core/Show
                assert= st $ assert-traits st calcit.core/Mappable
                assert= st $ assert-traits st calcit.core/Mappable calcit.core/Show
                assert= s $ assert-traits s calcit.core/Show
                assert= s $ assert-traits s calcit.core/Show calcit.core/Eq
                assert= opt $ assert-traits opt calcit.core/Mappable
                assert= p $ assert-traits p MyFoo
                ; Records satisfy the built-in Show trait through the shared record impls.
                assert= p $ assert-traits p calcit.core/Show
                ; Person has no implementation of the unrelated MyBar trait.
                assert= :true $ try
                  do (assert-traits p MyBar) :false
                  fn (e) (do :true)
                ; A same-named method from MyZapA must not satisfy the distinct MyZapB trait.
                assert= zp $ assert-traits zp MyZapA
                assert= :true $ try
                  do (assert-traits zp MyZapB) :false
                  fn (e) (do :true)
                assert= flag $ assert-traits flag calcit.core/Show calcit.core/Eq
                assert= keyword $ assert-traits keyword calcit.core/Show calcit.core/Eq
                assert= nothing $ assert-traits nothing calcit.core/Show calcit.core/Eq
              println "|  assert-traits: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-collection-traits $ %{} :CodeEntry (:doc "|Test Len/Empty/Contains traits for collections")
          :code $ quote
            defn test-collection-traits () (println "|Testing Collection traits (Len, Empty)...") (; Len trait)
              assert= 0 $ count ([])
              assert= 3 $ count ([] 1 2 3)
              assert= 5 $ count |hello
              assert= 2 $ count
                {} (:a 1) (:b 2)
              assert= 3 $ count (#{} 1 2 3)
              ; Empty trait
              assert= true $ empty? ([])
              assert= false $ empty? ([] 1)
              assert= true $ empty? ({})
              assert= false $ empty?
                {} $ :a 1
              assert= true $ empty? (#{})
              assert= false $ empty? (#{} 1)
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
                  m $ {} (:a 1)
                  s $ #{} 1 2
                  text |abc
                  tuple $ :: :demo 1
                  record $ %{} Person0 (:name |A)
                assert= 3 $ count-with-trait xs
                assert= 1 $ count-with-trait m
                assert= 2 $ count-with-trait s
                assert= 3 $ count-with-trait text
                assert= 2 $ count-with-trait tuple
                assert= 1 $ count-with-trait record
                assert= true $ contains-with-trait? xs 1
                assert= true $ contains-with-trait? m :a
                assert= true $ contains-with-trait? s 2
                assert= true $ contains-with-trait? text 1
                assert= true $ contains-with-trait? tuple 1
                assert= true $ contains-with-trait? record :name
                assert-traits xs Countable Contains
                assert-traits m Countable Contains
                assert-traits s Countable Contains
                assert-traits text Countable Contains
                assert-traits tuple Countable Contains
                assert-traits record Countable Contains
              println "|  Collection traits: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-compare-trait $ %{} :CodeEntry (:doc "|Test Compare trait")
          :code $ quote
            defn test-compare-trait () (println "|Testing Compare trait...") (; Number comparison)
              assert= true $ < 1 2
              assert= true $ > 2 1
              assert= true $ <= 1 1
              assert= true $ >= 2 2
              ; String comparison $ lexicographic
              assert= -1 $ &compare |apple |banana
              assert= 1 $ &compare |zebra |apple
              ; List comparison $ not yet implemented in compare form
              ; assert= :lt $ compare ([] 1 2) ([] 1 3)
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
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-cross-trait-method-conflict $ %{} :CodeEntry (:doc "|Test method conflict across traits")
          :code $ quote
            defn test-cross-trait-method-conflict () (println "|Testing cross-trait method conflict...")
              let
                  ; two different traits provide the same method name "`:zap`"
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
                assert= |zapB $ .zap pa
                assert= |zapA $ .zap pb
                assert= |zapB $ .zap ta
                assert= |zapA $ .zap tb
              println "|  cross-trait conflict: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-deftrait $ %{} :CodeEntry (:doc "|Test deftrait macro")
          :code $ quote
            defn test-deftrait () (println "|Testing deftrait macro...")
              assert= :trait $ type-of MyFoo
              let
                  Person $ impl-traits Person0 MyFooImpl
                  p $ %{} Person (:name |Alice)
                assert= "|foo Alice" $ .foo p
                println "|  deftrait: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-eq-trait $ %{} :CodeEntry (:doc "|Test Eq trait")
          :code $ quote
            defn test-eq-trait () (println "|Testing Eq trait...") (; Value equality)
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
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-explicit-trait-call $ %{} :CodeEntry (:doc "|Test explicit trait-call for disambiguation")
          :code $ quote
            defn test-explicit-trait-call () (println "|Testing explicit trait-call...")
              let
                  Person $ impl-traits Person0 MyZapAImpl MyZapBImpl
                  p $ %{} Person (:name |Alice)
                assert-traits p MyZapA MyZapB
                ; "`.zap`" follows normal dispatch $ last-wins for user impls
                assert= |zapB $ &trait-call MyZapB :zap p
                ; "`&trait-call`" selects by trait, bypassing "`.method`" ambiguity
                assert= |zapA $ &trait-call MyZapA :zap p
                assert= |zapB $ &trait-call MyZapB :zap p
              let
                  SinglePerson $ impl-traits Person0 MyZapAImpl
                  p $ %{} SinglePerson (:name |Bob)
                assert= |zapA $ &trait-call MyZapA :zap p
                assert= :true $ try
                  do (&trait-call MyZapB :zap p) :false
                  fn (e) (do :true)
              let
                  xs $ [] 1 2 3
                  flag true
                assert= 3 $ &trait-call calcit.core/Countable :count xs
                assert= |true $ &trait-call calcit.core/Show :show flag
                assert= true $ &trait-call calcit.core/Eq :eq? flag true
              let
                  t $ %:: DemoZap :demo 1
                assert-traits t MyZapA MyZapB
                assert= |zapB $ .zap t
                assert= |zapA $ &trait-call MyZapA :zap t
                assert= |zapB $ &trait-call MyZapB :zap t
              println "|  explicit trait-call: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-impl-precedence-order $ %{} :CodeEntry (:doc "|Test impl precedence order")
          :code $ quote
            defn test-impl-precedence-order () (println "|Testing impl precedence order...")
              let
                  ; impl-traits appends impls, so later ones override earlier ones
                  Person $ impl-traits Person0 MyFooImpl MyFooImpl2
                  p $ %{} Person (:name |Alice)
                assert= "|foo2 Alice" $ .foo p
              println "|  precedence: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-method-introspection $ %{} :CodeEntry (:doc "|Test runtime method introspection helpers")
          :code $ quote
            defn test-method-introspection () (println "|Testing method introspection...")
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
                  p $ %{} Person (:name |Alice)
                  ms2 $ &methods-of p
                assert= true $ includes? ms2 .foo
                assert= p $ &inspect-methods p |record
              let
                  ms3 $ &methods-of (impl-traits Person0 MyFooImpl)
                  ms4 $ &methods-of DemoBar
                  ms5 $ &methods-of MyFoo
                assert= true $ includes? ms3 .foo
                assert= true $ includes? ms4 .bar
                assert= true $ includes? ms5 .foo
              println "|  method introspection: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-option-result-map $ %{} :CodeEntry (:doc "|Test Mappable trait for Option/Result")
          :code $ quote
            defn test-option-result-map () (println "|Testing Option/Result Mappable...")
              let
                  opt-some $ %some 1
                  opt-none $ %none
                  res-ok $ %ok 1
                  res-err $ %err |oops
                assert-type opt-some Option
                assert-type opt-none Option
                assert-type res-ok Result
                assert-type res-err Result
                assert= (%some 2) (.map opt-some inc)
                assert= (%none) (.map opt-none inc)
                assert= (%ok 2) (.map res-ok inc)
                assert= (%err |oops) (.map res-err inc)
              let
                  opt-some $ %some 1
                  opt-none $ %none
                  res-ok $ %ok 1
                  res-err $ %err |oops
                assert= true $ .some? opt-some
                assert= true $ .none? opt-none
                assert= 1 $ .unwrap-or opt-some 9
                assert= 9 $ .unwrap-or opt-none 9
                assert= (%some 2)
                  .and-then opt-some $ fn (x)
                    %some $ inc x
                assert= (%none)
                  .and-then opt-none $ fn (x)
                    %some $ inc x
                assert= true $ .ok? res-ok
                assert= true $ .err? res-err
                assert= 1 $ .unwrap-or res-ok 9
                assert= 9 $ .unwrap-or res-err 9
                assert= (%ok 2)
                  .and-then res-ok $ fn (x)
                    %ok $ inc x
                assert= (%err |oops)
                  .and-then res-err $ fn (x)
                    %ok $ inc x
                assert= (%ok 1) (.map-err res-ok turn-tag)
                assert= (%err :oops) (.map-err res-err turn-tag)
              println "|  Option/Result map: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-show-trait $ %{} :CodeEntry (:doc "|Test Show trait for built-in types")
          :code $ quote
            defn test-show-trait () (println "|Testing Show trait...") (; All types should be showable)
              assert= |true $ str true
              assert= |false $ str false
              assert= |42 $ str 42
              assert= |hello $ str |hello
              assert= |:tag $ str :tag
              assert= "|([] 1 2 3)" $ str ([] 1 2 3)
              assert= "|({} (:a 1))" $ str
                {} $ :a 1
              ; assert= "|(#{} 1 2)" $ str (#{} 1 2)
              println "|  Show trait: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-tuple-impl-precedence-order $ %{} :CodeEntry (:doc "|Test tuple impl precedence order")
          :code $ quote
            defn test-tuple-impl-precedence-order () (println "|Testing tuple impl precedence order...")
              let
                  t $ %:: DemoBar :demo 1
                assert-traits t MyBar
                assert= |bar2 $ .bar t
              println "|  tuple precedence: ✓"
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-traits.main $ :require
