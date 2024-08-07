
{} (:package |test-record)
  :configs $ {} (:init-fn |test-record.main/main!) (:reload-fn |test-record.main/reload!)
  :files $ {}
    |test-record.main $ %{} :FileEntry
      :defs $ {}
        |BirdClass $ %{} :CodeEntry (:doc |)
          :code $ quote
            def BirdClass $ %{} BirdShape
              :show $ fn (self)
                println $ :name self
              :rename $ fn (self name) (assoc self :name name)
        |BirdShape $ %{} :CodeEntry (:doc |)
          :code $ quote
            def BirdShape $ new-record :BirdShape :show :rename
        |Cat $ %{} :CodeEntry (:doc |)
          :code $ quote (defrecord Cat :name :color)
        |Lagopus $ %{} :CodeEntry (:doc |)
          :code $ quote
            def Lagopus $ new-class-record BirdClass :Lagopus :name
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (test-record) (test-methods) (test-match) (test-polymorphism) (test-edn) (test-record-with) (do true)
        |test-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn ()
              let
                  content "|%{} :Lagopus (:name |La)"
                  data $ parse-cirru-edn content
                    {} $ :Lagopus Lagopus
                println |EDN: data
                assert= BirdClass $ &record:class data
              let
                  l1 $ %{} Lagopus (:name |LagopusA)
                println |EDN: $ format-cirru-edn l1
              let
                  Demo $ new-record :Demo :a :b :c :d
                  data $ %{} Demo (:a 1)
                    :b $ [] 2 3
                    :c 4
                    :d 5
                assert= "|%{} :Demo (:a 1) (:c 4) (:d 5)\n  :b $ [] 2 3" $ trim (format-cirru-edn data)
        |test-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record match")
              let
                  A $ new-record :A :a
                  B $ new-record :B :b
                  C $ new-record :C :c
                  a1 $ %{} A (:a 1)
                  b1 $ %{} B (:b 2)
                  c1 $ %{} C (:c 3)
                assert= 1 $ record-match a1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
                assert= 2 $ record-match b1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
                assert= :other $ record-match c1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
        |test-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record methods")
              assert= :Cat $ &record:get-name Cat
              let
                  kitty $ %{} Cat (:name |kitty) (:color :red)
                assert= :red $ &record:get kitty :color
                assert= true $ &record:matches? kitty Cat
                assert= (&record:to-map kitty) (&{} :name |kitty :color :red)
                assert= 2 $ &record:count kitty
                assert= true $ &record:contains? kitty :color
                assert= false $ &record:contains? kitty :age
                assert=
                  %{} kitty (:name |kitty) (:color :blue)
                  &record:assoc kitty :color :blue
                assert=
                  &record:from-map kitty $ &{} :name |kitty :color :red
                  %{} kitty (:name |kitty) (:color :red)
                &let
                  persian $ &record:extend-as kitty :Persian :age 10
                  assert= 10 $ &record:get persian :age
                  assert= :Persian $ &record:get-name persian
        |test-polymorphism $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test record polymorphism") (println Lagopus)
              let
                  l1 $ %{} Lagopus (:name |LagopusA)
                  a1 $ new-record :A :name
                println l1
                .show l1
                -> l1 (.rename |LagopusB) (.show)
                assert= (&record:class l1)
                  &record:class $ &record:with-class a1 BirdClass
        |test-record-with $ %{} :CodeEntry (:doc "|test record-with")
          :code $ quote
            fn () (log-title "|Testing record-with")
              let
                  Person $ new-record :City :name :age :position
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :hangzhou)
                  p2 $ record-with p1 (:age 21) (:position :shanghai)
                ; println |P2 p2
                assert= 20 $ get p1 :age
                assert= 21 $ get p2 :age
                assert= :hangzhou $ get p1 :position
                assert= :shanghai $ get p2 :position
                assert= |Chen $ get p2 :name
        |test-record $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record")
              let
                  Person $ new-record :Person :name :age :position
                  City $ new-record :City :name :province
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
                  p2 $ &%{} Person :name |Chen :age 20 :position :mainland
                  p0 $ &%{} Person :name nil :age nil :position nil
                  p3 $ &%{} Person :name |Chen :age 23 :position :mainland
                  c1 $ %{} City (:name |Shanghai) (:province |Shanghai)
                assert= Person p0
                assert= nil $ get Person :age
                assert= nil $ get Person 'age
                assert= nil $ get Person |age
                assert= 20 $ get p1 :age
                assert= 20 $ get p2 :age
                assert= 23 $ get p3 :age
                assert= 23 $ &record:get p3 :age
                assert= :record $ type-of p1
                assert= (&record:to-map p1)
                  {} (:name |Chen) (:age 20) (:position :mainland)
                assert= 21 $ get
                  &record:from-map Person $ {} (:name |Chen) (:age 21) (:position :mainland)
                  , :age
                assert= (keys p2) (#{} :age :name :position)
                assert-detect identity $ &record:matches? p1 p1
                assert-detect identity $ &record:matches? p1 p2
                assert-detect not $ &record:matches? p1 c1
                &let
                  p4 $ assoc p1 :age 30
                  assert= 20 $ get p1 :age
                  assert= 30 $ get p4 :age
                inside-js: $ js/console.log (to-js-data p1)
                assert-detect identity $ = p1 p1
                assert-detect identity $ = p1 p2
                assert-detect not $ = p1 p3
                assert-detect not $ = p1 c1
                assert=
                  %{} p1 (:age 23) (:name |Ye) (:position :mainland)
                  merge p1 $ {} (:age 23) (:name |Ye)
                assert=
                  %{} p1 (:age 23) (:name |Ye) (:position :mainland)
                  merge p1 $ {} (:age 23) (:name |Ye)
                assert-detect identity $ contains? p1 :name
                assert-detect not $ contains? p1 :surname
                assert= 3 $ count p1
                assert= 21 $ get (update p1 :age inc) :age
                assert= Cat $ new-record 'Cat :name :color
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-record.main $ :require
            [] util.core :refer $ [] log-title inside-js:
