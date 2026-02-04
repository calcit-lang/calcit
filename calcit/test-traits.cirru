
{} (:package |test-traits)
  :configs $ {} (:init-fn |test-traits.main/main!) (:reload-fn |test-traits.main/main!)
  :files $ {}
    |test-traits.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing built-in traits..."

              ; Test Show trait - all types should have it
              test-show-trait

              ; Test deftrait macro
              test-deftrait

              ; Test Eq trait
              test-eq-trait

              ; Test Compare trait
              test-compare-trait

              ; Test Add trait
              test-add-trait

              ; Test Len/Empty traits
              test-collection-traits

              println "|All trait tests passed!"
          :examples $ []

        |test-show-trait $ %{} :CodeEntry (:doc "|Test Show trait for built-in types")
          :code $ quote
            defn test-show-trait ()
              println "|Testing Show trait..."

              ; All types should be showable
              assert= | $ str nil
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
              :foo (:: :fn ('T) ('T) :string)
          :examples $ []

        |MyFooImpl $ %{} :CodeEntry (:doc "|Trait impl for deftrait test")
          :code $ quote
            defrecord! MyFooImpl
              :foo $ fn (p) (str "|foo " (:name p))
          :examples $ []

        |test-deftrait $ %{} :CodeEntry (:doc "|Test deftrait macro")
          :code $ quote
            defn test-deftrait ()
              println "|Testing deftrait macro..."
              assert= :trait $ type-of MyFoo
              let
                  Person0 $ new-record :Person :name
                  Person $ with-traits Person0 MyFooImpl
                  p $ %{} Person (:name |Alice)
                assert= "|foo Alice" $ .foo p
                println "|  deftrait: ✓"
          :examples $ []

        |test-eq-trait $ %{} :CodeEntry (:doc "|Test Eq trait")
          :code $ quote
            defn test-eq-trait ()
              println "|Testing Eq trait..."

              ; Value equality
              assert= true $ = nil nil
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

              println "|  Add trait: ✓"
          :examples $ []

        |test-collection-traits $ %{} :CodeEntry (:doc "|Test Len/Empty traits for collections")
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
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-traits.main $ :require
        :examples $ []
