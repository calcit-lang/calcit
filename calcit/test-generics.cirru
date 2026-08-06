
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-generics) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-generics.main/main!) (:mode :native) (:reload-fn 'test-generics.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-generics.main $ %{} :FileEntry
      :defs $ {}
        |Box $ %{} :CodeEntry (:doc "|Generic box struct")
          :code $ quote
            defstruct Box ([] 'T) (:value 'T)
          :examples $ []
          :schema $ :: 'Dynamic
        |Holder $ %{} :CodeEntry (:doc "|Generic holder wrapping Box")
          :code $ quote
            defstruct Holder ([] 'T)
              :box $ :: 'Box 'T
          :examples $ []
          :schema $ :: 'Dynamic
        |Node $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Node
              :next $ :: 'Optional Node
              :value 'Number
          :examples $ []
          :schema $ :: 'Dynamic
        |Pair $ %{} :CodeEntry (:doc "|Generic pair struct")
          :code $ quote
            defstruct Pair ([] 'T 'U) (:left 'T) (:right 'U)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ do (println "|Testing generics...") (println "|  - generic structs") (test-struct-generics) (println "|  - function generics and where-bounds") (test-recursive-struct) (test-fn-generics) (println "|Generics tests passed")
          :examples $ []
          :schema $ :: 'Dynamic
        |pair-right $ %{} :CodeEntry (:doc "|Return the right value from a generic pair")
          :code $ quote
            defn pair-right (pair) (:right pair)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'U)
              :args $ [] (:: 'test-generics.main/Pair 'T 'U)
              :generics $ [] 'T 'U
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |test-fn-generics $ %{} :CodeEntry (:doc |)
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
                    :return 'String
                  x .show
                n $ id2 1
                s $ id2 |hi
                shown-n $ show-id 1
                shown-s $ show-id |hi
              assert-type id $ :: 'Fn
                {} (:return 'T)
                  :generics $ [] 'T
                  :args $ [] 'T
              assert-type show-id $ :: 'Fn
                {}
                  :generics $ [] 'T
                  :where $ {} ('T Show)
                  :args $ [] 'T
                  :return 'String
              assert-type n 'Number
              assert-type s 'String
              assert= |1 shown-n
              assert= |hi shown-s
              &inspect-type id
              &inspect-type n
              &inspect-type s
              &inspect-type show-id
          :examples $ []
          :schema $ :: 'Dynamic
        |test-recursive-struct $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-recursive-struct () (println "|Testing recursive struct support...")
              let
                  leaf $ %{} Node (:next nil) (:value 1)
                  nested $ %{} Node (:next leaf) (:value 2)
                assert= 1 $ :value (:next nested)
                assert= 2 $ :value nested
                println "|Recursive struct support passed"
          :examples $ []
          :schema $ :: 'Dynamic
        |test-struct-generics $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-struct-generics () $ do (println "|Testing generic struct support...")
              assert= 2 $ unbox (&%{} Box :value 2)
              assert= |hi $ pair-right (&%{} Pair :left 1 :right |hi)
              assert-type
                unbox $ &%{} Box :value 2
                , 'Number
              assert-type
                pair-right $ &%{} Pair :left 1 :right |hi
                , 'String
              &inspect-type $ &%{} Pair :left 1 :right |hi
              &inspect-type $ &%{} Box :value 2
              &inspect-type $ &%{} Holder :box (&%{} Box :value 2)
              println "|Generic struct support passed"
          :examples $ []
          :schema $ :: 'Dynamic
        |unbox $ %{} :CodeEntry (:doc "|Return value from a generic box")
          :code $ quote
            defn unbox (box) (:value box)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] (:: 'test-generics.main/Box 'T)
              :generics $ [] 'T
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-generics.main)
