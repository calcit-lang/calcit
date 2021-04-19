
{} (:package |test-macro)
  :configs $ {} (:init-fn |test-macro.main/main!) (:reload-fn |test-macro.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-macro.main $ {}
      :ns $ quote
        ns test-macro.main $ :require
          [] util.core :refer $ [] log-title inside-nim:
      :defs $ {}

        |test-cond $ quote
          defn test-cond ()
            let
                compare-x $ fn (x)
                  cond
                    (&> x 10) "|>10"
                    (&> x 5) "|>5"
                    true "|<=5"
              assert= (compare-x 11) "|>10"
              assert= (compare-x 10) "|>5"
              assert= (compare-x 6) "|>5"
              assert= (compare-x 4) "|<=5"

        |test-case $ quote
          defn test-case ()
            log-title "|Testing case"

            let
                detect-x $ fn (x)
                  case x
                    1 "|one"
                    2 "|two"
                    x "|else"
              assert= (detect-x 1) "|one"
              assert= (detect-x 2) "|two"
              assert= (detect-x 3) "|else"

            inside-nim:

              &reset-gensym-index!

              assert=
                macroexpand-all $ quote
                  case (+ 1 2)
                    1 |one
                    2 |two
                    3 |three
                quote
                  &let (v__1 (+ 1 2))
                    if (&= v__1 1) |one
                      if (&= v__1 2) |two
                        if (&= v__1 3) |three nil
              assert=
                macroexpand $ quote
                  case (+ 1 2)
                    1 |one
                    2 |two
                    3 |three
                quote
                  &let (v__2 (+ 1 2))
                    &case v__2 nil
                      1 |one
                      2 |two
                      3 |three
              assert=
                macroexpand $ quote
                  &case v__2 nil
                    1 |one
                    2 |two
                    3 |three
                quote
                  if (&= v__2 1) |one
                    &case v__2 nil
                      2 |two
                      3 |three


        |test-expr-in-case $ quote
          defn test-expr-in-case ()
            assert= |5
              case (+ 1 4)
                (+ 2 0) |2
                (+ 2 1) |3
                (+ 2 2) |4
                (+ 2 3) |5
                (+ 2 4) |6

        |test-thread-macros $ quote
          defn test-thread-macros ()
            log-title "|Testing thread macros"

            inside-nim:
              assert=
                macroexpand $ quote $ -> a b c
                quote (c (b a))

              assert=
                macroexpand $ quote $ -> a (b) c
                quote (c (b a))

              assert=
                macroexpand $ quote $ -> a (b c)
                quote (b a c)

              assert=
                macroexpand $ quote $ -> a (b c) (d e f)
                quote (d (b a c) e f)

              assert=
                macroexpand $ quote $ ->> a b c
                quote (c (b a))

              assert=
                macroexpand $ quote $ ->> a (b) c
                quote (c (b a))

              assert=
                macroexpand $ quote $ ->> a (b c)
                quote (b c a)

              assert=
                macroexpand $ quote $ ->> a (b c) (d e f)
                quote (d e f (b c a))

              assert=
                macroexpand $ quote $ ->% a
                quote a

              assert=
                macroexpand $ quote $ ->% a (+ % 1) (* % 2)
                quote $ let
                    % a
                    % (+ % 1)
                  * % 2

            assert= 35
              ->% 3 (+ % 4) (* % 5)

            assert= 36
              ->% 3 (+ % %) (* % %)

        |test-lambda $ quote
          fn ()
            log-title "|Testing lambda macro"

            inside-nim:
              assert-detect identity $ contains-symbol?
                quote $ add $ + 1 %
                , '%

              assert-detect not $ contains-symbol?
                quote $ add $ + 1 2
                , '%

            assert=
              map (\ + 1 %) (range 3)
              range 1 4

            assert=
              map-indexed (\ [] % (&str %2)) (range 3)
              []
                [] 0 |0
                [] 1 |1
                [] 2 |2

            inside-nim:
              assert=
                macroexpand-all $ quote (\ + 2 %)
                quote $ defn f% (? % %2) (+ 2 %)

              assert=
                macroexpand-all $ quote $ \ x
                quote $ defn f% (? % %2) (x)

              assert=
                macroexpand-all $ quote $ \ + x %
                quote $ defn f% (? % %2) (+ x %)

              assert=
                macroexpand-all $ quote $ \ + x % %2
                quote $ defn f% (? % %2) (+ x % %2)

              assert=
                macroexpand $ quote $ \. x x
                quote $ defn f_x (x) x

              assert=
                macroexpand $ quote $ \. x.y x
                quote $ defn f_x (x) (defn f_y (y) x)

              assert=
                macroexpand $ quote $ \. x.y (echo x y) x
                quote $ defn f_x (x) (defn f_y (y) (do (echo x y) x))

            echo "|evaluating lambda alias"

            assert= 2
              (\. x x) 2

            assert= 2
              ((\. x.y x) 2) 3

            assert= 2
              ((\. x.y (echo "|inside lambda alias" x y) x) 2) 3

        |test-gensym $ quote
          fn ()
            inside-nim:
              &reset-gensym-index!
              assert= (gensym) 'G__1
              assert=
                gensym 'a
                , 'a__2
              assert=
                gensym |a
                , 'a__3

        |test-with-log $ quote
          fn ()
            log-title "|Testing with-log"

            &reset-gensym-index!

            inside-nim:
              assert=
                macroexpand $ quote $ with-log $ + 1 2
                quote $ &let
                  v__1 $ + 1 2
                  echo (format-to-lisp (quote $ + 1 2)) |=> v__1
                  , v__1

            assert=
              with-log $ + 1 2
              , 3

            ; echo $ macroexpand $ quote $ call-with-log + 1 2 3 4
            assert= 10 $ call-with-log + 1 2 3 4


            inside-nim:
              &reset-gensym-index!

              assert=
                macroexpand $ quote
                  defn-with-log f1 (a b) (+ a b)
                quote
                  defn f1 (a b)
                    &let
                      f1 (defn f1 (a b) (+ a b))
                      call-with-log f1 a b

              ; echo $ macroexpand $ quote
                defn-with-log f1 (a b) (+ a b)

            let
                f2 $ defn-with-log f1 (a b) (+ a b)
              assert= 7 $ f2 3 4
              assert= 11 $ f2 & ([] 5 6)

        |test-with-cpu-time $ quote
          fn ()
            log-title "|Testing with-cpu-time"

            inside-nim:
              &reset-gensym-index!

              assert=
                macroexpand $ quote $ with-cpu-time $ + 1 2
                quote
                  let
                      started__1 $ cpu-time
                      v__2 $ + 1 2
                    echo |[cpu-time]
                      quote $ + 1 2
                      , |=>
                      format-number (&* 1000 (&- (cpu-time) started__1)) 3
                      , |ms
                    , v__2

            assert=
              with-cpu-time $ + 1 2
              , 3
            assert=
              with-cpu-time $ &+ 1 2
              , 3

        |test-assert $ quote
          fn ()
            log-title "|Assert in different order"
            assert (= 1 1) |string
            assert |string (= 1 1)

        |test-extract $ quote
          fn ()
            log-title "|Extract map via keywords"

            inside-nim:
              &reset-gensym-index!

              assert=
                macroexpand $ quote $ let{} (a b) o
                  + a b
                quote $ &let (result__1 o)
                  assert (str "|expected map for destructing: " result__1) (map? result__1)
                  let
                      a $ :a result__1
                      b $ :b result__1
                    + a b

            &let
              base $ {}
                :a 5
                :b 6
              assert= 11 $ let{} (a b) base
                + a b

            inside-nim:
              assert=
                macroexpand $ quote
                  let-destruct ([] a b) ([] 3 4)
                    + a b
                quote $ let[] (a b) ([] 3 4) (+ a b)

              assert=
                macroexpand $ quote
                  let-destruct ({} a b) ({,} :a 3 :b 4)
                    + a b
                quote $ let{} (a b) ({,} :a 3 :b 4)
                  + a b

            assert=
              [] 3 4 5 6
              let-sugar
                  ([] a b) ([] 3 4)
                  ({} c d) ({,} :c 5 :d 6)
                [] a b c d

        |test-detector $ quote
          fn ()
            log-title "|Detector function"

            inside-nim:
              &reset-gensym-index!

              assert=
                macroexpand $ quote $ assert-detect fn? $ fn () 1
                quote
                  &let
                    v__1 (fn () 1)
                    if (fn? v__1) nil
                      do (echo)
                        echo (quote (fn () 1)) "|does not satisfy:" (quote fn?) "| <--------"
                        echo "|  value is:" v__1
                        raise "|Not satisfied in assertion!"

        |main! $ quote
          defn main! ()
            log-title "|Testing cond"
            test-cond

            test-case

            log-title "|Testing expr in case"
            test-expr-in-case

            test-thread-macros

            test-lambda

            log-title "|Testing gensym"
            test-gensym

            test-with-log

            test-with-cpu-time

            test-assert

            test-extract

            test-detector

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
