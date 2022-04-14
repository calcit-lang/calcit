
{} (:package |test-js)
  :configs $ {} (:init-fn |test-js.main/main!) (:reload-fn |test-js.main/reload!)
  :files $ {}
    |test-js.main $ {}
      :ns $ quote
        ns test-js.main $ :require
          |os :as os
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            println
            println title
            println

        |test-js $ quote
          fn ()
            js/console.log $ js/Math.pow 4 4
            js/console.log $ * js/Math.PI 2
            when
              = |number (js/typeof 1)
              js/console.log "|is a Number"

            .!log js/console |demo
            js/console.log "|Dates in difference syntax" (.!now js/Date)
            js/console.log $ .-PI js/Math

            js/console.log $ aget js/Math |PI
            let
                a js/{}
              aset a |name |demo
              js/console.log a
            js/console.log $ os/arch
            println $ {}
              :n 1
              :js $ js-array 1 2 3
            js/console.log $ {}
              :n 1
              :js $ js-array 1 2 3
            eprintln "|a simulated error for eprintln"
            js/console.log $ :: 'quote $ + 1 2 3

            assert= 0 $ .-length $ new js/Array
            assert= 7 $ .-length $ new js/Array (+ 3 4)

            let
                a $ new js/Object
              set! (.-a a) 2
              assert= (.-a a) 2

              set! (.-a-b a) 3
              assert= (.-a-b a) 3

            assert= 2 $ aget (to-js-data $ [] 1 2 3) 1

            assert-detect identity $ instance? js/Number (new js/Number 1)
            assert-detect not $ instance? js/String (new js/Number 1)

            assert=
              [] 1 ([] 2 3) (:: :quote ([] 'a 'b))
              to-calcit-data $ js-array 1 ([] 2 3) (:: :quote ([] 'a 'b))

            assert=
              &{} |a 1 :b 2 |c $ [] 3 4
              to-calcit-data $ &js-object |a 1 |:b 2 :c $ [] 3 4

        |test-let-example $ quote
          fn ()
            log-title "|Testing code emitting of using let"
            let
                a 1
                b 2
                c $ + a b
                b 4
                d 5
              assert= 13 $ + a b c d

        |test-collection $ quote
          fn ()
            log-title "|Testing quick collection syntax"

            &let
              a $ js-array 1 2 3 4
              assert= 4 $ .-length a
              assert= 1 $ aget a 0
              assert= 4 $ aget a 3
              assert= nil $ aget a 4
              assert= 2 (.-1 a)
            &let
              b $ js-object (:a 1) (|b 2) (:c 3)
              assert= 1 $ .-a b
              assert= 2 $ .-b b
              assert= 3 $ .-c b
              assert= 2 $ aget b |b

        |test-async $ quote
          fn ()
            let
                f1 $ fn ()
                  hint-fn async
                  new js/Promise $ fn (resolve reject)
                    js/setTimeout
                      fn ()
                        println "|async code finished after 1s"
                        resolve true
                      , 1000
                f2 $ fn ()
                  hint-fn async
                  js-await $ f1
              f2

        |main! $ quote
          defn main! ()
            log-title "|Testing js"
            test-js
            test-let-example

            test-collection

            test-async

            when (> 1 2)
              raise (str "|error of math" 2 1)
              raise "|base error"

            =
              {} (:a 1)
              w-js-log ({} (:a 1))

            =
              {} (:a 1)
              wo-js-log ({} (:a 1))

            w-js-log "|log demo"

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
