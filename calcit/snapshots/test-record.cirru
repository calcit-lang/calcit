
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
                Person $ new-record 'Person :name :age :position
                City $ new-record 'City :name :province
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

              assert= nil (&get Person :age)
              assert= nil (&get Person 'age)
              assert= nil (&get Person |age)

              assert= 20 (&get p1 :age)
              assert= 20 (&get p2 :age)
              assert= 23 (&get p3 :age)

              assert= :record $ type-of p1
              assert=
                turn-map p1
                {} (:name |Chen) (:age 20) (:position :mainland)

              assert= 21
                &get
                  make-record Person $ {}
                    :name |Chen
                    :age 21
                    :position :mainland
                  , :age

              assert=
                keys p2
                #{} 'position 'age 'name

              assert-detect identity $ relevant-record? p1 p1
              assert-detect identity $ relevant-record? p1 p2
              assert-detect not $ relevant-record? p1 c1

              &let
                p4 $ assoc p1 :age 30
                assert= 20 $ &get p1 :age
                assert= 30 $ &get p4 :age

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
              assert=
                nth p1 1
                [] 'name |Chen

              assert= 21
                get (update p1 :age inc) :age

              assert= Cat
                new-record 'Cat :name :color

        |Cat $ quote
          defrecord Cat :name :color

        |main! $ quote
          defn main! ()
            test-record

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
