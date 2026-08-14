
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-struct) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-struct.main/main!) (:mode :native) (:reload-fn 'test-struct.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-struct.main $ %{} 'FileEntry
      :defs $ {}
        |A $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct A $ :a 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |A0 $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct A0 $ :name 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |B $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct B $ :b 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |BirdImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defimpl BirdImpl BirdTrait
              .show $ fn (self)
                println $ &struct:get self :name
              .rename $ fn (self name) (assoc self :name name)
          :examples $ []
          :schema $ :: 'Dynamic
        |BirdShape $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct BirdShape (:show 'Fn) (:rename 'Fn)
          :examples $ []
          :schema $ :: 'Dynamic
        |BirdTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote
            deftrait BirdTrait (.show :fn) (.rename :fn)
          :examples $ []
          :schema $ :: 'Dynamic
        |C $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct C $ :c 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |Cat $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Cat (:name 'String) (:color 'Tag)
          :examples $ []
          :schema $ :: 'Dynamic
        |City $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct City (:name 'String) (:province 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |Demo $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Demo (:a 'Dynamic) (:b 'Dynamic) (:c 'Dynamic) (:d 'Dynamic)
          :examples $ []
          :schema $ :: 'Dynamic
        |Lagopus $ %{} 'CodeEntry (:doc |)
          :code $ quote
            def Lagopus $ impl-traits Lagopus0 BirdImpl
          :examples $ []
          :schema $ :: 'Dynamic
        |Lagopus0 $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Lagopus0 $ :name (:: 'Optional 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |MapLiteralStore $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct MapLiteralStore $ {} (:text 'String)
          :examples $ []
          :schema $ :: 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |map-literal-fields)
              :code $ quote
                let
                    store $ MapLiteralStore :text |ok
                  assert= |ok $ :text store
        |Person $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Person
              :name $ :: 'Optional 'String
              :age $ :: 'Optional 'Number
              :position $ :: 'Optional 'Tag
          :examples $ []
          :schema $ :: 'Dynamic
        |Point2D $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Point2D (:x 'Number) (:y 'Number)
          :examples $ []
          :schema $ :: 'Dynamic
        |check-point-type $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn check-point-type (p) (struct? p)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'test-struct.main/Point2D
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (test-struct) (test-methods) (test-match) (test-polymorphism) (test-edn) (test-struct-with) (test-partial-struct) (test-loose-struct-rewrite) (test-map-to-struct) (test-postfix) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |read-asserted-map-literal-store $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-asserted-map-literal-store (source)
              let
                  store source
                assert-type store test-struct.main/MapLiteralStore
                :text store
          :examples $ []
          :schema $ :: 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |assert-type-statement-narrows-struct)
              :code $ quote
                assert= |ok $ read-asserted-map-literal-store (MapLiteralStore :text |ok)
        |read-let-asserted-map-literal-store $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-let-asserted-map-literal-store (source)
              let
                  store $ assert-type source test-struct.main/MapLiteralStore
                :text store
          :examples $ []
          :schema $ :: 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |assert-type-expression-narrows-struct)
              :code $ quote
                assert= |ok $ read-let-asserted-map-literal-store (MapLiteralStore :text |ok)
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ println |reloaded
          :examples $ []
          :schema $ :: 'Dynamic
        |sum-point $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn sum-point (p)
              &+ (:x p) (:y p)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'test-struct.main/Point2D
        |test-edn $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn ()
              let
                  content "|%{} :Lagopus0 (:name |La)"
                  data $ parse-cirru-edn content
                    {} $ :Lagopus0
                      %{} Lagopus $ :name nil
                println |EDN: data
                assert= true $ any? (&struct:impls data)
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
                assert= "|%{} 'Demo (:a 1) (:c 4) (:d 5)\n  :b $ [] 2 3" $ trim (format-cirru-edn data)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-loose-struct-rewrite $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing loose-to-struct rewrite")
              assert= 30 $ sum-point (?{} :x 10 :y 20)
              assert= true $ check-point-type (?{} :x 10 :y 20)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-to-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing map-to-struct rewrite")
              assert= 30 $ sum-point
                {} (:x 10) (:y 20)
              assert= true $ check-point-type
                {} (:x 10) (:y 20)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-match $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing struct match")
              let
                  a1 $ %{} A (:a 1)
                  b1 $ %{} B (:b 2)
                  c1 $ %{} C (:c 3)
                assert= 1 $ struct-match a1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
                assert= 2 $ struct-match b1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
                assert= :other $ struct-match c1
                  A aa $ :a aa
                  B bb $ :b bb
                  _ o (println |others) :other
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing struct methods")
              &let
                kitty $ %{} Cat (:name |kitty) (:color :red)
                assert= :Cat $ &struct:get-name kitty
                assert= :red $ &struct:get kitty :color
                assert= true $ = (&struct:definition kitty) Cat
                assert= true $ struct-def? (&struct:definition kitty)
                assert= true $ &struct:matches? kitty
                  %{} (&struct:definition kitty) (:name |kitty) (:color :red)
                assert= (&struct:to-map kitty) (&{} :name |kitty :color :red)
                assert= 2 $ &struct:count kitty
                assert=
                  &struct:get kitty $ &struct:field-tag kitty 0
                  &struct:nth kitty 0
                assert=
                  &struct:get kitty $ &struct:field-tag kitty 1
                  &struct:nth kitty 1
                assert= true $ &struct:contains? kitty (&struct:field-tag kitty 0)
                assert= true $ &struct:contains? kitty (&struct:field-tag kitty 1)
                assert= true $ &struct:contains? kitty :color
                assert= false $ &struct:contains? kitty :age
                assert=
                  %{} Cat (:name |kitty) (:color :blue)
                  &struct:assoc kitty :color :blue
                assert=
                  &struct:from-map Cat $ &{} :name |kitty :color :red
                  %{} Cat (:name |kitty) (:color :red)
                &let
                  persian $ &struct:extend-as kitty :Persian :age 10
                  assert= 10 $ &struct:get persian :age
                  assert= :Persian $ &struct:get-name persian
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-partial-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing partial struct")
              let
                  p1 $ %{}? Person (:name |Chen)
                  p2 $ %{}? Person (:name |Chen) (:age 20) (:position :mainland)
                  p3 $ %{}? Person (:age 31)
                assert= |Chen $ :name p1
                assert= nil $ :age p1
                assert= nil $ :position p1
                assert= 20 $ :age p2
                assert= nil $ :name p3
                assert= 31 $ :age p3
                assert= nil $ :position p3
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-polymorphism $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test struct polymorphism") (println Lagopus)
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
                  assert= (&struct:impls l1) (&struct:impls a1r)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-postfix $ %{} 'CodeEntry (:doc "|test postfix syntax")
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
        |test-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing struct")
              let
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
                  p2 $ &%{} Person :name |Chen :age 20 :position :mainland
                  p0 $ &%{} Person :name nil :age nil :position nil
                  p3 $ &%{} Person :name |Chen :age 23 :position :mainland
                  c1 $ %{} City (:name |Shanghai) (:province |Shanghai)
                assert= true $ = (&struct:definition p0) Person
                assert= nil $ :age p0
                assert= nil $ :name p0
                assert= nil $ :position p0
                assert= 20 $ :age p1
                assert= 20 $ :age p2
                assert= 23 $ :age p3
                assert= 23 $ &struct:get p3 :age
                assert= :struct $ type-of p1
                assert= (&struct:to-map p1)
                  {} (:name |Chen) (:age 20) (:position :mainland)
                assert= 21 $ :age
                  &struct:from-map Person $ {} (:name |Chen) (:age 21) (:position :mainland)
                assert= (keys p2) (#{} :age :name :position)
                assert-detect identity $ &struct:matches? p1 p1
                assert-detect identity $ &struct:matches? p1 p2
                assert-detect not $ &struct:matches? p1 c1
                &let
                  p4 $ assoc p1 :age 30
                  assert= 20 $ :age p1
                  assert= 30 $ :age p4
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
                assert= 21 $ :age
                  update p1 :age $ fn (age)
                    if (nil? age) 1 $ inc age
                assert= 20 $ :age p1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
              :features $ #{} :js-ffi
        |test-struct-with $ %{} 'CodeEntry (:doc "|test struct-with")
          :code $ quote
            fn () (log-title "|Testing struct-with")
              let
                  p1 $ %{} Person (:name |Chen) (:age 20) (:position :hangzhou)
                  p2 $ struct-with p1 (:age 21) (:position :shanghai)
                ; println |P2 p2
                assert= 20 $ :age p1
                assert= 21 $ :age p2
                assert= :hangzhou $ :position p1
                assert= :shanghai $ :position p2
                assert= |Chen $ :name p2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-struct.main $ :require
            util.core :refer $ log-title inside-js:
