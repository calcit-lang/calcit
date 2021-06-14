
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ [] |./test-cond.cirru |./test-gynienic.cirru |./test-json.cirru
      , |./test-lens.cirru |./test-list.cirru |./test-macro.cirru |./test-map.cirru
      , |./test-math.cirru |./test-recursion.cirru |./test-set.cirru
      , |./test-string.cirru |./test-ternary.cirru |./test-js.cirru |./test-record.cirru
      , |./util.cirru
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require
          [] test-cond.main :as test-cond
          [] test-gynienic.main :as test-gynienic
          [] test-json.main :as test-json
          [] test-lens.main :as test-lens
          [] test-list.main :as test-list
          [] test-macro.main :as test-macro
          [] test-map.main :as test-map
          [] test-math.main :as test-math
          [] test-recursion.main :as test-recursion
          [] test-set.main :as test-set
          [] test-string.main :as test-string
          [] test-ternary.main :as test-ternary
          [] test-js.main :as test-js
          [] test-record.main :as test-record
          util.core :refer $ log-title inside-eval: inside-js:
      :defs $ {}
        |test-keyword $ quote
          defn test-keyword ()
            ; assert "|keyword function" $ =
              :a ({} (:a 1))
              , 1
            ; inside-eval:
              &let
                base $ {} (:a 1)
                assert= 1 $ base :a

            assert-detect identity $ < :a :b
            assert-detect identity $ < :aa :ab
            assert-detect not $ > :a :b
            assert-detect not $ > :aa :ab

            inside-eval:
              assert= ([] 1)
                .map
                  [] $ &{} :a 1
                  , :a

        |test-id $ quote
          fn ()
            assert= 9 $ count $ generate-id! 9
            assert= |aaaaa $ generate-id! 5 |a

        |test-detects $ quote
          defn test-detects ()
            assert-detect fn? $ fn () 1
            assert-detect not $ bool? $ fn () 1

            assert-detect fn? &=
            assert-detect macro? cond

            assert-detect set? $ #{} 1 2 3

            assert= 1 (either nil 1)
            assert= 2 (either 2 1)
            assert= nil (either nil nil)

            assert= 2 $ either 2
              raise "|should not be called"

            assert= 2 (def x 2)

            assert= false $ and true true false
            assert= false $ and true false true
            assert= true $ and true true true

            assert= false $ or false false false
            assert= true $ or false true false
            assert= true $ or false false true

            assert=
              or true (raise "|raise in or")
              , true
            assert=
              and false (raise "|raise in and")
              , false

            assert= 2
              when-not (> 1 2) 1 2

            assert= 1
              if-not (> 2 1) 2 1
            assert= nil
              if-not (> 2 1) 2

            assert-detect identity
              /= 1 2
            assert-detect identity
              not= 1 2

            assert= true $ some-in? ({,} :a 1) ([] :a)
            assert= false $ some-in? ({,} :a 1) ([] :b)
            assert= false $ some-in? nil ([] :b)
            assert= false $ some-in? nil ([])

            assert= true $ some-in? ({,} :a ([] 1)) ([] :a 0)
            assert= false $ some-in? ({,} :a ([] 1)) ([] :a 1)

            assert= false $ some-in? ([] 1 2 3) ([] :a)

        |test-time $ quote
          fn ()
            log-title "|Testing time"
            inside-js:
              ; "|println only since CI uses a different timezone"
              println 1605024000 $ parse-time |2020-11-11
              assert= "|2020-11-10T16:01:40.000Z"
                format-time 1605024100
              assert= "|2020-11-10T16:01:40.123Z"
                format-time 1605024100.1234
              echo $ format-time (now!) "|yyyy-MM-dd HH:mm:ss ffffff"

            inside-eval:
              echo |time: $ format-time (now!) "|%Y-%m-%d %H:%M:%S %z"
              assert= 1417176009000
                parse-time "|2014-11-28 21:00:09 +09:00" "|%Y-%m-%d %H:%M:%S %z"
              assert= "|2014-11-28 12:00:09 +0000"
                format-time 1417176009000 "|%Y-%m-%d %H:%M:%S %z"

        |test-if $ quote
          fn ()
            log-title "|Testing if with nil"
            assert= (if false 1) (if nil 1)
            assert= (if false 1 2) (if nil 1 2)

        |test-display-stack $ quote
          fn ()
            log-title "|Testing display stack"
            display-stack "|show stack here"

        |test-cirru-parser $ quote
          fn ()
            log-title "|Testing Cirru parser"
            assert=
              parse-cirru "|def f (a b) $ + a b"
              [] $ [] |def |f ([] |a |b)
                [] |+ |a |b

            assert=
              parse-cirru "|{,} :a 1 :b false"
              [] $ [] |{,} |:a |1 |:b |false

            assert=
              parse-cirru-edn "|{} (:a 1) (:b ([] 3 |4 nil))"
              {}
                :a 1
                :b $ [] 3 |4 nil

            assert= "|[] |a |b $ [] |c |d"
              trim $ write-cirru-edn $ [] |a |b $ [] |c |d
            assert= "|a b $ c d"
              trim $ write-cirru $ [] $ [] |a |b $ [] |c |d

        |test-fn $ quote
          fn ()
            log-title "|Testing fn"

            &let
              empty-f $ fn ()
              assert= nil (empty-f)

            &let
              coll-f $ fn (& xs) xs
              assert=
                [] 1 2 3 4 5
                coll-f 1 & ([] 2 3 4) 5

        |test-arguments $ quote
          fn ()
            log-title "|Testing arguments"
            let
                f1 $ fn (a ? b c) $ [] a b c

              assert= (f1 :a) ([] :a nil nil)
              assert= (f1 :a :b) ([] :a :b nil)
              assert= (f1 :a :b :c) ([] :a :b :c)

        |test-try $ quote
          fn ()
            log-title "|Testing try"
            assert= :true
              try
                do (echo "|inside try") :true
                fn (error)

            assert= :false
              try
                do (echo "|inside false try")
                  raise "|error intented" ([] :demo)
                  , :true
                fn (error)
                  do
                    echo "|Caught error:" error
                    , :false

            echo "|Finished testing try"

        |test-fn-eq $ quote
          fn ()
            log-title "|Testing equality of functions"
            let
                a $ fn (x) x
                b $ fn (x) x
              assert= a a
              assert= b b
              assert= false (&= a b)

        |*ref-demo $ quote
          defatom *ref-demo 0
        |test-refs $ quote
          fn ()
            log-title "|Testing refs"
            assert= 0 @*ref-demo
            add-watch *ref-demo :change $ fn (prev current)
              println "|change happened:" prev current
            reset! *ref-demo 2
            remove-watch *ref-demo :change
            assert= 2 @*ref-demo
            assert= :ref (type-of *ref-demo)

        |%Num $ quote
          defrecord %Num :inc :show
        |Num $ quote
          def Num $ %{} %Num
            :inc $ fn (x)
              update x 1 inc
            :show $ fn (x)
              str $ &tuple:nth x 1

        |test-method $ quote
          fn ()
            log-title "|Testing method"

            let
                a $ :: Num 0
              assert=
                :: Num 2
                -> a .inc .inc
              assert= |1
                -> a .inc .show

        |test-tuple $ quote
          fn ()
            log-title "|Testing tuple"

            assert= :tuple (type-of (:: :a :b))
            assert= :a (nth (:: :a :b) 0)
            assert= :b (nth (:: :a :b) 1)
            assert= 2 (count (:: :a :b))

        |reload! $ quote
          defn reload! () nil

        |main! $ quote
          defn main! ()
            inside-js:
              load-console-formatter!
          
            log-title "|Testing keyword function"
            test-keyword

            log-title "|Testing detects"
            test-detects

            inside-eval:
              log-title "|Testing id"
              test-id

            test-time

            test-if

            test-display-stack

            test-cirru-parser

            test-fn

            test-macro/main!

            test-arguments

            test-try

            test-fn-eq

            test-refs

            test-method

            test-tuple

            inside-eval:
              test-gynienic/main!

            test-cond/main!
            test-json/main!
            test-lens/main!
            test-list/main!
            test-map/main!
            test-math/main!
            test-recursion/main!
            test-set/main!
            test-string/main!
            test-record/main!

            inside-js:
              test-js/main!

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
