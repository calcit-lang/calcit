
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-js)
  :configs $ {} (:init-fn |test-js.main/main!) (:reload-fn |test-js.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-js.main $ %{} :FileEntry
      :defs $ {}
        |load-data-code $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defmacro load-data-code (s)
              &data-to-code $ parse-cirru-edn s
          :examples $ []
        |log-title $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (log-title "|Testing js") (test-js) (test-let-example) (test-collection) (test-async) (test-async-in-data) (test-data-gen) (test-regexp) (test-property) (test-tag-keys)
              when (> 1 2)
                raise $ str "|error of math" 2 1
                raise "|base error"
              =
                {} $ :a 1
                w-js-log $ {} (:a 1)
              =
                {} $ :a 1
                wo-js-log $ {} (:a 1)
              w-js-log "|log demo"
              test-for-await
              test-case-async
              test-return-raw-code
              do true
          :examples $ []
        |test-async $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            fn () $ let
                f1 $ fn ()
                  hint-fn $ {} (:async true)
                  new js/Promise $ fn (resolve reject)
                    js/setTimeout
                      fn () (println "|async code finished after 200ms") (resolve true)
                      , 200
                f2 $ fn ()
                  hint-fn $ {} (:async true)
                  js-await $ f1
                  assert= true $ if true
                    js-await $ f1
                  let
                      a $ js-await (f1)
                    assert= true a
              f2
          :examples $ []
        |test-async-in-data $ %{} :CodeEntry (:doc "|async fn inside data. if wrong, it will be a syntax error from await outside async") (:schema nil)
          :code $ quote
            fn () $ let
                timeout $ fn (ms)
                  new js/Promise $ fn (resolve reject) (js/setTimeout resolve ms)
                f 0
                f $ let
                    b $ fn ()
                      hint-fn $ {} (:async true)
                      let
                          a 1
                          a $ js-await (timeout 200)
                        assert= nil a
                  b
              js/console.log "|a promise from nested let" f
          :examples $ []
        |test-case-async $ %{} :CodeEntry (:doc "|case async") (:schema nil)
          :code $ quote
            fn ()
              hint-fn $ {} (:async true)
              let
                  a $ {} (:a 1)
                  b $ :a a
                  ret $ js-await
                    case-default b
                      new js/Promise $ fn (resolve _reject)
                        js/setTimeout
                          fn () $ resolve |one
                          , 100
                      1 $ new js/Promise
                        fn (resolve reject) (resolve |one)
                      2 $ new js/Promise
                        fn (resolve reject) (resolve |two)
                assert= ret |one
          :examples $ []
        |test-collection $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            fn () (log-title "|Testing quick collection syntax")
              &let
                a $ js-array 1 2 3 4
                assert= 4 $ .-length a
                assert= 1 $ aget a 0
                assert= 4 $ aget a 3
                assert= js/undefined $ aget a 4
                assert= 2 $ .-1 a
              &let
                b $ js-object (:a 1) (|b 2) (:c 3)
                assert= 1 $ .-a b
                assert= 2 $ .-b b
                assert= 3 $ .-c b
                assert= 2 $ aget b |b
              let
                  c nil
                  d $ js-object (:a 2)
                  e $ js-array 1 2 3
                assert= nil $ .?-a c
                assert= nil $ .?-1 c
                assert= 2 $ .?-a d
                assert= 2 $ .?-1 e
              let
                  caller $ fn () 2
                  c $ js-object
                  d $ js-object (:f caller)
                  e $ js-array caller
                  f $ js-array
                assert= nil $ .?!f c
                assert= 2 $ .?!f d
                assert= nil $ .?!2 f
                assert= 2 $ .?!0 e
          :examples $ []
        |test-data-gen $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            fn () (log-title "|Testing code gen from Cirru Edn")
              assert=
                :: :code $ &cirru-nth (parse-cirru "|+ 1 2") 0
                load-data-code "|:: :code $ quote $ + 1 2"
          :examples $ []
        |test-for-await $ %{} :CodeEntry (:doc "|for await") (:schema nil)
          :code $ quote
            fn ()
              hint-fn $ {} (:async true)
              let
                  gen $ &raw-code "|async function* genDemo() { yield 1; yield 2; yield 3; } "
                  ret $ js-await
                    js-for-await (gen)
                      fn (item)
                        new js/Promise $ fn (resolve _reject)
                          js/setTimeout $ fn () (resolve item)
                assert= 3 ret
          :examples $ []
        |test-js $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            fn ()
              js/console.log $ js/Math.pow 4 4
              js/console.log $ * js/Math.PI 2
              when
                = |number $ js/typeof 1
                js/console.log "|is a Number"
              .!log js/console |demo
              js/console.log "|Dates in difference syntax" $ .!now js/Date
              js/console.log $ .-PI js/Math
              js/console.log $ aget js/Math |PI
              let
                  a js/{}
                aset a |name |demo
                js/console.log a
              js/console.log $ os/arch
              println $ {} (:n 1)
                :js $ js-array 1 2 3
              js/console.log $ {} (:n 1)
                :js $ js-array 1 2 3
              eprintln "|a simulated error for eprintln"
              js/console.log $ :: 'quote (+ 1 2 3)
              js/console.log $ parse-cirru "|+ 1 2 3"
              js/console.log $ parse-cirru "|defn f (a b) (+ x y) (* x y)"
              println $ parse-cirru "|+ 1 2 3"
              assert= 0 $ .-length (new js/Array)
              assert= 7 $ .-length
                new js/Array $ + 3 4
              let
                  a $ new js/Object
                set! (.-a a) 2
                assert= (.-a a) 2
                assert= a.-a 2
                set! (.-a-b a) 3
                assert= (.-a-b a) 3
              ; js/console.log $ range 1000
              ; js/console.log $ repeat
                {} $ :a (range 10)
                , 400
              assert/deepEqual
                to-js-data $ [] 1 2 3
                js-array 1 2 3
              assert/deepEqual
                to-js-data $ :: :a 1 2
                js-array |a 1 2
              assert-detect identity $ instance? js/Number (new js/Number 1)
              assert-detect not $ instance? js/String (new js/Number 1)
              assert=
                [] 1 ([] 2 3)
                  :: :quote $ [] 'a 'b
                to-calcit-data $ js-array 1 ([] 2 3)
                  :: :quote $ [] 'a 'b
              assert=
                &{} |a 1 :b 2 |c $ [] 3 4
                to-calcit-data $ &js-object |a 1 |:b 2 :c ([] 3 4)
          :examples $ []
        |test-let-example $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            fn () (log-title "|Testing code emitting of using let")
              let
                  a 1
                  b 2
                  c $ + a b
                  b 4
                  d 5
                assert= 13 $ + a b c d
              ; "a special case variable shadowing of `b`"
              let
                  b -1
                  a $ loop
                      xs $ []
                      b 0
                    if (>= b 5) xs $ recur (conj xs b) (inc b)
                assert= a $ [] 0 1 2 3 4
                assert= b -1
          :examples $ []
        |test-property $ %{} :CodeEntry (:doc "|try property ops") (:schema nil)
          :code $ quote
            fn () $ let
                a $ js-object
              js-set a |b 1
              assert= 1 $ js-get a |b
              js-delete a |b
              assert= nil $ js-get a |b
          :examples $ []
        |test-regexp $ %{} :CodeEntry (:doc "|try raw code and regexp") (:schema nil)
          :code $ quote
            fn () $ let
                pattern $ &raw-code |/^\d+$/
              js/console.log pattern
              assert= true $ .!test pattern |12
              assert= false $ .!test pattern |xy
              assert= true $ pattern.!test |12
              assert= false $ pattern.!test |xy
          :examples $ []
        |test-return-raw-code $ %{} :CodeEntry (:doc "|return with &raw-code") (:schema nil)
          :code $ quote
            fn () $ let
                a $ js-array 1 2
                f $ fn (t)
                  if t (.-0 a) (&raw-code |a[1])
              assert= (f true) 1
              assert= (f false) 2
          :examples $ []
        |test-tag-keys $ %{} :CodeEntry (:doc "|tag keys for js") (:schema nil)
          :code $ quote
            fn ()
              assert= |a_b $ turn-string :a_b
              assert= |a-b $ turn-string :a-b
              assert= |a?b $ turn-string :a?b
              assert= |ab! $ turn-string :ab!
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns test-js.main $ :require (|os :as os) (|assert :as assert)
        :examples $ []
