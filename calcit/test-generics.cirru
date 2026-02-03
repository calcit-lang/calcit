
{} (:package |test-generics)
  :configs $ {} (:init-fn |test-generics.main/main!) (:reload-fn |test-generics.main/reload!)
  :files $ {}
    |test-generics.main $ %{} :FileEntry
      :defs $ {}
        |Box $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Box ('T)
              :value 'T

        |Pair $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Pair ('A 'B)
              :left 'A
              :right 'B

        |Holder $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Holder ('T)
              :box (:: Box 'T)

        |Wrapped $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Wrapped
              :pair (:: Pair :number :string)
              :none

        |test-struct-generics $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-struct-generics ()
              let
                  p $ %{} (new-record :Pair :left :right) (:left 1) (:right |hi)
                  b $ %{} (new-record :Box :value) (:value 2)
                  h $ %{} (new-record :Holder :box) (:box b)
                assert-type p (:: Pair :number :string)
                assert-type b (:: Box :number)
                assert-type h (:: Holder :number)
                &inspect-type p
                &inspect-type b
                &inspect-type h

        |test-fn-generics $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-fn-generics ()
              let
                  id $ fn (x)
                    hint-fn (type-vars 'T)
                    do x
                  id2 $ fn (x)
                    hint-fn (type-vars 'T) (return-type 'T)
                    do x
                  n $ id2 1
                  s $ id2 |hi
                assert-type id (:: :fn ('T) ('T) 'T)
                assert-type n :number
                assert-type s :string
                &inspect-type id
                &inspect-type n
                &inspect-type s

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing generics..."
              test-struct-generics
              test-fn-generics
              println "|Generics tests passed"

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-generics.main)
