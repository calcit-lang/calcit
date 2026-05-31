
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-generics)
  :configs $ {} (:init-fn |test-generics.main/main!) (:reload-fn |test-generics.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-generics.main $ %{} :FileEntry
      :defs $ {}
        |Box $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Box $ :value :number
          :examples $ []
        |Holder $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Holder $ :box Box
          :examples $ []
        |Pair $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Pair (:left :number) (:right :string)
          :examples $ []
        |Wrapped $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defenum Wrapped (:pair Pair) (:none)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () $ do
              println "|Testing generics..."
              println "|  - data structure baseline"
              test-struct-generics
              println "|  - function generics and where-bounds"
              test-fn-generics
              println "|Generics tests passed"
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |test-fn-generics $ %{} :CodeEntry (:doc |) (:schema :dynamic)
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
                show-id $ fn (x)
                  hint-fn $ {}
                    :generics $ [] 'T
                    :where $ {} ('T Show)
                    :args $ [] 'T
                    :return :string
                  .show x
                n $ id2 1
                s $ id2 |hi
                shown-n $ show-id 1
                shown-s $ show-id |hi
              assert-type id $ :: :fn
                {} (:return 'T)
                  :generics $ [] 'T
                  :args $ [] 'T
              assert-type show-id $ :: :fn
                {}
                  :generics $ [] 'T
                  :where $ {} ('T Show)
                  :args $ [] 'T
                  :return :string
              assert-type n :number
              assert-type s :string
              assert= |1 shown-n
              assert= |hi shown-s
              &inspect-type id
              &inspect-type n
              &inspect-type s
              &inspect-type show-id
          :examples $ []
        |test-struct-generics $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-struct-generics () $ do
              println "|Testing generic data structure baseline..."
              let
                  p $ %{} Pair (:left 1) (:right |hi)
                  b $ %{} Box (:value 2)
                  h $ %{} Holder (:box b)
                assert-type p Pair
                assert-type b Box
                assert-type h Holder
                &inspect-type p
                &inspect-type b
                &inspect-type h
              println "|Generic data structure baseline passed"
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-generics.main)
