
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-record)
  :configs $ {} (:init-fn |test-record.main/main!) (:reload-fn |test-record.main/reload!) (:version |0.0.0)
    :modules $ [] |./util.cirru
  :entries $ {}
  :files $ {}
    |test-record.main $ %{} :FileEntry
      :defs $ {}
        |A $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct A $ :a :dynamic
          :examples $ []
        |A0 $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct A0 $ :name :string
          :examples $ []
        |B $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct B $ :b :dynamic
          :examples $ []
        |BirdImpl $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defimpl BirdImpl BirdTrait
              .show $ fn (self)
                println $ :name self
              .rename $ fn (self name) (assoc self :name name)
          :examples $ []
        |BirdShape $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct BirdShape (:show :fn) (:rename :fn)
          :examples $ []
        |BirdTrait $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            deftrait BirdTrait (.show :fn) (.rename :fn)
          :examples $ []
        |C $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct C $ :c :dynamic
          :examples $ []
        |Cat $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Cat (:name :string) (:color :tag)
          :examples $ []
        |City $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct City (:name :string) (:province :string)
          :examples $ []
        |Demo $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Demo (:a :dynamic) (:b :dynamic) (:c :dynamic) (:d :dynamic)
          :examples $ []
        |Lagopus $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            def Lagopus $ impl-traits Lagopus0 BirdImpl
          :examples $ []
        |Lagopus0 $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Lagopus0 $ :name (:optional :string)
          :examples $ []
        |Person $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Person
              :name $ :optional :string
              :age $ :optional :number
              :position $ :optional :tag
          :examples $ []
        |Point2D $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defstruct Point2D (:x :number) (:y :number)
          :examples $ []
        |check-point-type $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn check-point-type (p) (record? p)
          :examples $ []
          :schema $ :: :fn
            {} (:return :bool)
              :args $ [] 'test-record.main/Point2D
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (test-record) (test-methods) (test-match) (test-polymorphism) (test-edn) (test-record-with) (test-partial-record) (test-loose-record-rewrite) (test-map-to-record) (do true)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () $ println |reloaded
          :examples $ []
        |sum-point $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn sum-point (p)
              &+ (:x p) (:y p)
          :examples $ []
          :schema $ :: :fn
            {} (:return :number)
              :args $ [] 'test-record.main/Point2D
        |test-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn ()
              let
                  content "|%{} :Lagopus0 (:name |La)"
                  data $ parse-cirru-edn content
                    {} $ :Lagopus0
                      %{} Lagopus $ :name nil
                println |EDN: data
                assert= true $ any? (&record:impls data)
                  fn (impl)
                    = (&impl:origin impl) BirdTrait
              let
                  l1 $ %{} Lagopus (:name |LagopusA)
                println |EDN: $ format-cirru-edn l1
              let
                  data $ %{} Demo (:a 1)
                    :b $ [] 2 3
                    :c 4
                    :d 5
                assert= "|%{} :Demo (:a 1) (:c 4) (:d 5)\n  :b $ [] 2 3" $ trim (format-cirru-edn data)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |test-loose-record-rewrite $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            fn () (log-title "|Testing loose-record-to-struct rewrite")
              assert= 30 $ sum-point (?{} :x 10 :y 20)
              assert= true $ check-point-type (?{} :x 10 :y 20)
          :examples $ []
        |test-map-to-record $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            fn () (log-title "|Testing map-to-record rewrite")
              assert= 30 $ sum-point
                {} (:x 10) (:y 20)
              assert= true $ check-point-type
                {} (:x 10) (:y 20)
          :examples $ []
        |test-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record match")
              let
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
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |test-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record methods")
              &let
                kitty $ %{} Cat (:name |kitty) (:color :red)
                assert= :Cat $ &record:get-name kitty
                assert= :red $ &record:get kitty :color
                assert= true $ = (&record:struct kitty) Cat
                assert= true $ struct? (&record:struct kitty)
                assert= true $ &record:matches? kitty
                  %{} (&record:struct kitty) (:name |kitty) (:color :red)
                assert= (&record:to-map kitty) (&{} :name |kitty :color :red)
                assert= 2 $ &record:count kitty
                assert=
                  &record:get kitty $ &record:field-tag kitty 0
                  &record:nth kitty 0
                assert=
                  &record:get kitty $ &record:field-tag kitty 1
                  &record:nth kitty 1
                assert= true $ &record:contains? kitty (&record:field-tag kitty 0)
                assert= true $ &record:contains? kitty (&record:field-tag kitty 1)
                assert= true $ &record:contains? kitty :color
                assert= false $ &record:contains? kitty :age
                assert=
                  %{} Cat (:name |kitty) (:color :blue)
                  &record:assoc kitty :color :blue
                assert=
                  &record:from-map Cat $ &{} :name |kitty :color :red
                  %{} Cat (:name |kitty) (:color :red)
                &let
                  persian $ &record:extend-as kitty :Persian :age 10
                  assert= 10 $ &record:get persian :age
                  assert= :Persian $ &record:get-name persian
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |test-partial-record $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing partial record")
              let
                  p1 $ %{}? Person (:name |Chen)
                  p2 $ %{}? Person (:name |Chen) (:age 20) (:position :mainland)
                  p3 $ %{}? Person (:age 31)
                assert= |Chen $ get p1 :name
                assert= nil $ get p1 :age
                assert= nil $ get p1 :position
                assert= 20 $ get p2 :age
                assert= nil $ get p3 :name
                assert= 31 $ get p3 :age
                assert= nil $ get p3 :position
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |test-polymorphism $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test record polymorphism") (println Lagopus)
              let
                  l1 $ %{} Lagopus (:name |LagopusA)
                  a1 A0
                  a2 $ impl-traits a1 BirdImpl
                  a1r $ %{} a2 (:name |Demo)
                  l1t l1
                assert-traits l1t BirdTrait
                let
                    l2 $ .rename l1t |LagopusB
                    l2t l2
                  assert-traits l2t BirdTrait
                  println l1
                  .show l1t
                  .show l2t
                  assert= (&record:impls l1) (&record:impls a1r)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |test-record $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record")
              let
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
                  p2 $ &%{} Person :name |Chen :age 20 :position :mainland
                  p0 $ &%{} Person :name nil :age nil :position nil
                  p3 $ &%{} Person :name |Chen :age 23 :position :mainland
                  c1 $ %{} City (:name |Shanghai) (:province |Shanghai)
                assert= true $ = (&record:struct p0) Person
                assert= nil $ get p0 :age
                assert= nil $ get p0 :name
                assert= nil $ get p0 :position
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
                  %{} Person (:age 23) (:name |Ye) (:position :mainland)
                  merge p1 $ {} (:age 23) (:name |Ye)
                assert=
                  %{} Person (:age 23) (:name |Ye) (:position :mainland)
                  merge p1 $ {} (:age 23) (:name |Ye)
                assert-detect identity $ contains? p1 :name
                assert-detect not $ contains? p1 :surname
                assert= 3 $ count p1
                assert= 21 $ get (update p1 :age inc) :age
                assert= 20 $ :age p1
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
              :features $ #{} :js-ffi
        |test-record-with $ %{} :CodeEntry (:doc "|test record-with")
          :code $ quote
            fn () (log-title "|Testing record-with")
              let
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :hangzhou)
                  p2 $ record-with p1 (:age 21) (:position :shanghai)
                ; println |P2 p2
                assert= 20 $ get p1 :age
                assert= 21 $ get p2 :age
                assert= :hangzhou $ get p1 :position
                assert= :shanghai $ get p2 :position
                assert= |Chen $ get p2 :name
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-record.main $ :require
            util.core :refer $ log-title inside-js:
