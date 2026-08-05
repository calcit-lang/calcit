
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-record) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-record.main/main!) (:mode :native) (:reload-fn 'test-record.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-record.main $ %{} :FileEntry
      :defs $ {}
        |A $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct A $ :a 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |A0 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct A0 $ :name 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |B $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct B $ :b 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |BirdImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl BirdImpl BirdTrait
              .show $ fn (self)
                println $ :name self
              .rename $ fn (self name) (assoc self :name name)
          :examples $ []
          :schema $ :: 'Dynamic
        |BirdShape $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct BirdShape (:show 'Fn) (:rename 'Fn)
          :examples $ []
          :schema $ :: 'Dynamic
        |BirdTrait $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait BirdTrait (.show :fn) (.rename :fn)
          :examples $ []
          :schema $ :: 'Dynamic
        |C $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct C $ :c 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |Cat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Cat (:name 'String) (:color 'Tag)
          :examples $ []
          :schema $ :: 'Dynamic
        |City $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct City (:name 'String) (:province 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |Demo $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Demo (:a 'Dynamic) (:b 'Dynamic) (:c 'Dynamic) (:d 'Dynamic)
          :examples $ []
          :schema $ :: 'Dynamic
        |Lagopus $ %{} :CodeEntry (:doc |)
          :code $ quote
            def Lagopus $ impl-traits Lagopus0 BirdImpl
          :examples $ []
          :schema $ :: 'Dynamic
        |Lagopus0 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Lagopus0 $ :name (:: 'Optional 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |Person $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Person
              :name $ :: 'Optional 'String
              :age $ :: 'Optional 'Number
              :position $ :: 'Optional 'Tag
          :examples $ []
          :schema $ :: 'Dynamic
        |Point2D $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Point2D (:x 'Number) (:y 'Number)
          :examples $ []
          :schema $ :: 'Dynamic
        |check-point-type $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn check-point-type (p) (record? p)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'test-record.main/Point2D
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (test-record) (test-methods) (test-match) (test-polymorphism) (test-edn) (test-record-with) (test-partial-record) (test-loose-record-rewrite) (test-map-to-record) (test-postfix) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ println |reloaded
          :examples $ []
          :schema $ :: 'Dynamic
        |sum-point $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn sum-point (p)
              &+ (:x p) (:y p)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
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
                    = (impl-origin impl) (%some BirdTrait)
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
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-loose-record-rewrite $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing loose-record-to-struct rewrite")
              assert= 30 $ sum-point (?{} :x 10 :y 20)
              assert= true $ check-point-type (?{} :x 10 :y 20)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-to-record $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing map-to-record rewrite")
              assert= 30 $ sum-point
                {} (:x 10) (:y 20)
              assert= true $ check-point-type
                {} (:x 10) (:y 20)
          :examples $ []
          :schema $ :: 'Dynamic
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
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing record methods")
              &let
                kitty $ %{} Cat (:name |kitty) (:color :red)
                assert= :Cat $ &record:get-name kitty
                assert= :red $ &record:get kitty :color
                assert= true $ = (record-struct kitty) Cat
                assert= true $ struct? (record-struct kitty)
                assert= true $ &record:matches? kitty
                  %{} (record-struct kitty) (:name |kitty) (:color :red)
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
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-partial-record $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing partial record")
              let
                  p1 $ %{}? Person (:name |Chen)
                  p2 $ %{}? Person (:name |Chen) (:age 20) (:position :mainland)
                  p3 $ %{}? Person (:age 31)
                assert= (%some |Chen) (get p1 :name)
                assert= (%some nil) (get p1 :age)
                assert= (%some nil) (get p1 :position)
                assert= (%some 20) (get p2 :age)
                assert= (%some nil) (get p3 :name)
                assert= (%some 31) (get p3 :age)
                assert= (%some nil) (get p3 :position)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
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
                    l2 $ l1t .rename |LagopusB
                    l2t l2
                  assert-traits l2t BirdTrait
                  println l1
                  l1t .show
                  l2t .show
                  assert= (&record:impls l1) (&record:impls a1r)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-postfix $ %{} :CodeEntry (:doc "|test postfix syntax")
          :code $ quote
            fn () (log-title "|Testing postfix syntax")
              let
                  p $ &%{} Point2D :x 10 :y 20
                assert= 10 $ p :x
                assert= 20 $ p :y
              let
                  ffi-point $ unsafe-coerce (?{} :x 30 :y 40) Point2D
                assert= 30 $ ffi-point :x
                assert= 40 $ ffi-point :y
              let
                  l1 $ %{} Lagopus (:name |LagopusA)
                assert= |LagopusA $ :name l1
                let
                    l2 $ l1 .rename |LagopusB
                  assert-type l2 Lagopus
                  assert= |LagopusB $ :name l2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
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
                assert= true $ = (record-struct p0) Person
                assert= (%some nil) (get p0 :age)
                assert= (%some nil) (get p0 :name)
                assert= (%some nil) (get p0 :position)
                assert= (%some 20) (get p1 :age)
                assert= (%some 20) (get p2 :age)
                assert= (%some 23) (get p3 :age)
                assert= 23 $ &record:get p3 :age
                assert= :record $ type-of p1
                assert= (&record:to-map p1)
                  {} (:name |Chen) (:age 20) (:position :mainland)
                assert= (%some 21)
                  get
                    &record:from-map Person $ {} (:name |Chen) (:age 21) (:position :mainland)
                    , :age
                assert= (keys p2) (#{} :age :name :position)
                assert-detect identity $ &record:matches? p1 p1
                assert-detect identity $ &record:matches? p1 p2
                assert-detect not $ &record:matches? p1 c1
                &let
                  p4 $ assoc p1 :age 30
                  assert= (%some 20) (get p1 :age)
                  assert= (%some 30) (get p4 :age)
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
                assert= (%some 21)
                  get
                    update p1 :age $ fn (age)
                      if (nil? age) 1 $ inc age
                    , :age
                assert= 20 $ :age p1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
              :features $ #{} :js-ffi
        |test-record-with $ %{} :CodeEntry (:doc "|test record-with")
          :code $ quote
            fn () (log-title "|Testing record-with")
              let
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :hangzhou)
                  p2 $ record-with p1 (:age 21) (:position :shanghai)
                ; println |P2 p2
                assert= (%some 20) (get p1 :age)
                assert= (%some 21) (get p2 :age)
                assert= (%some :hangzhou) (get p1 :position)
                assert= (%some :shanghai) (get p2 :position)
                assert= (%some |Chen) (get p2 :name)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-record.main $ :require
            util.core :refer $ log-title inside-js:
