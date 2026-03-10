
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-generics)
  :configs $ {} (:init-fn |test-generics.main/main!) (:reload-fn |test-generics.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-generics.main $ %{} :FileEntry
      :defs $ {}
        |Box $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Box $ :value :number
          :examples $ []
        |Holder $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Holder $ :box Box
          :examples $ []
        |Pair $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Pair (:left :number) (:right :string)
          :examples $ []
        |Wrapped $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Wrapped
              :pair $ :: Pair :number :string
              :none
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (println "|Testing generics...") (test-struct-generics) (test-fn-generics) (println "|Generics tests passed")
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |test-fn-generics $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-fn-generics () $ let
                id $ fn (x)
                  hint-fn $ {}
                    :generics $ [] 'T
                  do x
                id2 $ fn (x)
                  hint-fn $ {}
                    :generics $ [] 'T
                    :return 'T
                  do x
                n $ id2 1
                s $ id2 |hi
              assert-type id $ :: :fn
                {} (:return 'T)
                  :generics $ [] 'T
                  :args $ [] 'T
              assert-type n :number
              assert-type s :string
              &inspect-type id
              &inspect-type n
              &inspect-type s
          :examples $ []
        |test-struct-generics $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-struct-generics () $ let
                p $ %{} Pair (:left 1) (:right |hi)
                b $ %{} Box (:value 2)
                h $ %{} Holder (:box b)
              assert-type p Pair
              assert-type b Box
              assert-type h Holder
              &inspect-type p
              &inspect-type b
              &inspect-type h
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-generics.main)
