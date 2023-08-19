
{} (:package |test-record)
  :configs $ {} (:init-fn |test-record.main/main!) (:reload-fn |test-record.main/reload!)
  :files $ {}
    |test-record.main $ {}
      :ns $ quote
        ns test-record.main $ :require
          [] util.core :refer $ [] log-title inside-js:
      :defs $ {}
        |test-record $ quote
          fn ()
            log-title "|Testing record"
            let
                Person $ new-record :Person :name :age :position
                City $ new-record :City :name :province
                p1 $ %{} Person
                  :name |Chen
                  :age 20
                  :position :mainland
                p2 $ &%{} Person :name |Chen :age 20 :position :mainland
                p0 $ &%{} Person :name nil :age nil :position nil
                p3 $ &%{} Person :name |Chen :age 23 :position :mainland
                c1 $ %{} City
                  :name |Shanghai
                  :province |Shanghai

              assert= Person p0

              assert= nil (get Person :age)
              assert= nil (get Person 'age)
              assert= nil (get Person |age)

              assert= 20 (get p1 :age)
              assert= 20 (get p2 :age)
              assert= 23 (get p3 :age)
              assert= 23 (&record:get p3 :age)

              assert= :record $ type-of p1
              assert=
                &record:to-map p1
                {} (:name |Chen) (:age 20) (:position :mainland)

              assert= 21
                get
                  &record:from-map Person $ {}
                    :name |Chen
                    :age 21
                    :position :mainland
                  , :age

              assert=
                keys p2
                #{} :age :name :position

              assert-detect identity $ &record:matches? p1 p1
              assert-detect identity $ &record:matches? p1 p2
              assert-detect not $ &record:matches? p1 c1

              &let
                p4 $ assoc p1 :age 30
                assert= 20 $ get p1 :age
                assert= 30 $ get p4 :age

              inside-js:
                js/console.log $ to-js-data p1

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

              assert= 21
                get (update p1 :age inc) :age

              assert= Cat
                new-record 'Cat :name :color

        |Cat $ quote
          defrecord Cat :name :color

        |test-methods $ quote
          fn ()
            log-title "|Testing record methods"

            assert= :Cat
              &record:get-name Cat

            let
                kitty $ %{} Cat
                  :name |kitty
                  :color :red

              assert= :red
                &record:get kitty :color
              assert= true
                &record:matches? kitty Cat
              assert=
                &record:to-map kitty
                &{} :name |kitty :color :red
              assert= 2
                &record:count kitty
              assert= true
                &record:contains? kitty :color
              assert= false
                &record:contains? kitty :age
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

        |test-match $ quote
          fn ()
            log-title "|Testing record match"
            let
                A $ new-record :A :a
                B $ new-record :B :b
                C $ new-record :C :c
                a1 $ %{} A (:a 1)
                b1 $ %{} B (:b 2)
                c1 $ %{} C (:c 3)
              assert= 1
                record-match a1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
              assert= 2
                record-match b1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
              assert= :other
                record-match c1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other

        |BirdShape $ quote
          def BirdShape $ new-record :BirdShape :show :rename

        |BirdClass $ quote
          def BirdClass $ %{} BirdShape
            :show $ fn (self)
              println $ :name self
            :rename $ fn (self name)
              assoc self :name name

        |Lagopus $ quote
          def Lagopus $ new-class-record BirdClass :Lagopus :name

        |test-polymorphism $ quote
          fn ()
            log-title "|Test record polymorphism"

            println Lagopus

            let
                l1 $ %{} Lagopus
                  :name |LagopusA
                a1 $ new-record :A :name
              println l1
              .show l1
              -> l1
                .rename |LagopusB
                .show

              assert=
                &record:class l1
                &record:class $ &record:with-class a1 BirdClass

        |main! $ quote
          defn main! ()
            test-record

            test-methods

            test-match

            test-polymorphism

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
