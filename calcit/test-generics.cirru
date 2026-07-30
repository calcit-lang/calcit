
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-generics) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |test-generics.main/main!) (:mode :native) (:reload-fn |test-generics.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-generics.main $ %{} :FileEntry
      :defs $ {}
        |Box $ %{} :CodeEntry (:doc "|Generic box struct") (:schema :dynamic)
          :code $ quote
            defstruct Box ([] 'T) (:value 'T)
          :examples $ []
        |Holder $ %{} :CodeEntry (:doc "|Generic holder wrapping Box") (:schema :dynamic)
          :code $ quote
            defstruct Holder ([] 'T)
              :box $ :: 'Box 'T
          :examples $ []
        |Pair $ %{} :CodeEntry (:doc "|Generic pair struct") (:schema :dynamic)
          :code $ quote
            defstruct Pair ([] 'T 'U) (:left 'T) (:right 'U)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () $ do (println "|Testing generics...") (println "|  - generic structs") (test-struct-generics) (println "|  - function generics and where-bounds") (test-fn-generics) (println "|Generics tests passed")
          :examples $ []
        |pair-right $ %{} :CodeEntry (:doc "|Return the right value from a generic pair")
          :code $ quote
            defn pair-right (pair) (:right pair)
          :examples $ []
          :schema $ :: :fn
            {} (:return 'U)
              :args $ [] (:: 'test-generics.main/Pair 'T 'U)
              :generics $ [] 'T 'U
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
            defn test-struct-generics () $ do (println "|Testing generic struct support...")
              assert= 2 $ unbox (&%{} Box :value 2)
              assert= |hi $ pair-right (&%{} Pair :left 1 :right |hi)
              assert-type
                unbox $ &%{} Box :value 2
                , :number
              assert-type
                pair-right $ &%{} Pair :left 1 :right |hi
                , :string
              &inspect-type $ &%{} Pair :left 1 :right |hi
              &inspect-type $ &%{} Box :value 2
              &inspect-type $ &%{} Holder :box (&%{} Box :value 2)
              println "|Generic struct support passed"
          :examples $ []
        |unbox $ %{} :CodeEntry (:doc "|Return value from a generic box")
          :code $ quote
            defn unbox (box) (:value box)
          :examples $ []
          :schema $ :: :fn
            {} (:return 'T)
              :args $ [] (:: 'test-generics.main/Box 'T)
              :generics $ [] 'T
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-generics.main)
