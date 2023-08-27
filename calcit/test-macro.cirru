
{} (:package |test-macro)
  :configs $ {} (:init-fn |test-macro.main/main!) (:reload-fn |test-macro.main/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-macro.main $ {}
      :configs $ {} (:extension nil)
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing cond") (test-cond) (test-case) (log-title "|Testing expr in case") (test-expr-in-case) (test-thread-macros) (test-lambda) (test-gensym) (test-w-log) (test-with-cpu-time) (test-assert) (test-extract) (test-detector) (test-if-let) (test-flipped) (test-misc) (do true)
        |test-assert $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Assert in different order")
              assert (= 1 1) |string
              assert |string $ = 1 1
        |test-case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-case () (log-title "|Testing case")
              let
                  detect-x $ fn (x)
                    case x (1 |one) (2 |two) (x |else)
                assert= (detect-x 1) |one
                assert= (detect-x 2) |two
                assert= (detect-x 3) |else
              inside-eval: (&reset-gensym-index!)
                assert=
                  macroexpand-all $ quote
                    case (+ 1 2) (1 |one) (2 |two) (3 |three)
                  quasiquote $ ~&let
                    v__1 $ + 1 2
                    ~if (~&= v__1 1) |one $ ~if (~&= v__1 2) |two
                      ~if (~&= v__1 3) |three nil
                assert=
                  macroexpand $ quote
                    case (+ 1 2) (1 |one) (2 |two) (3 |three)
                  quote $ &let
                    v__2 $ + 1 2
                    &case v__2 nil (1 |one) (2 |two) (3 |three)
                assert=
                  macroexpand $ quote
                    &case v__2 nil (1 |one) (2 |two) (3 |three)
                  quote $ if (&= v__2 1) |one
                    &case v__2 nil (2 |two) (3 |three)
        |test-cond $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-cond () $ let
                compare-x $ fn (x)
                  cond
                      &> x 10
                      , |>10
                    (&> x 5) |>5
                    true |<=5
              assert= (compare-x 11) |>10
              assert= (compare-x 10) |>5
              assert= (compare-x 6) |>5
              assert= (compare-x 4) |<=5
        |test-detector $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Detector function")
              inside-eval: (&reset-gensym-index!)
                assert=
                  macroexpand $ quote
                    assert-detect fn? $ fn () 1
                  quote $ &let
                    v__1 $ fn () 1
                    if (fn? v__1) nil $ &let () (eprintln)
                      eprintln
                        format-to-lisp $ quote
                          fn () 1
                        , "|does not satisfy:"
                          format-to-lisp $ quote fn?
                          , "| <--------"
                      eprintln "|  value is:" v__1
                      raise "|Not satisfied in assertion!"
        |test-expr-in-case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-expr-in-case () $ assert= |5
              case (+ 1 4)
                  + 2 0
                  , |2
                (+ 2 1) |3
                (+ 2 2) |4
                (+ 2 3) |5
                (+ 2 4) |6
        |test-extract $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Extract map via tags")
              inside-eval: (&reset-gensym-index!)
                assert=
                  macroexpand $ quote
                    let{} (a b) o $ + a b
                  quote $ &let (result__1 o)
                    assert (str "|expected map for destructing: " result__1) (map? result__1)
                    let
                        a $ :a result__1
                        b $ :b result__1
                      + a b
              &let
                base $ {} (:a 5) (:b 6)
                assert= 11 $ let{} (a b) base (+ a b)
              inside-eval:
                assert=
                  macroexpand $ quote
                    let-destruct ([] a b) ([] 3 4) (+ a b)
                  quote $ let[] (a b) ([] 3 4) (+ a b)
                assert=
                  macroexpand $ quote
                    let-destruct ({} a b) ({,} :a 3 :b 4) (+ a b)
                  quote $ let{} (a b) ({,} :a 3 :b 4) (+ a b)
                &reset-gensym-index!
                assert=
                  macroexpand-all $ quote
                    let[] (a b) ([] 1 2) (+ a b)
                  quasiquote $ ~&let
                    v__1 $ ~[] 1 2
                    ~&let
                      a $ ~&list:nth v__1 0
                      ~&let
                        b $ ~&list:nth v__1 1
                        + a b
                assert=
                  macroexpand-all $ quote
                    let[] (a b) xs $ + a b
                  quasiquote $ ~&let
                    a $ ~&list:nth xs 0
                    ~&let
                      b $ ~&list:nth xs 1
                      + a b
                assert=
                  macroexpand-all $ quote
                    cond
                        = a 1
                        , |one
                      true |other
                  quasiquote $ ~if (= a 1) |one |other
              assert= ([] 3 4 5 6)
                let-sugar
                      [] a b
                      [] 3 4
                    ({} c d) ({,} :c 5 :d 6)
                  [] a b c d
        |test-flipped $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title |flipped)
              assert=
                flipped [] 1 2 $ + 3 4
                [] 7 2 1
        |test-gensym $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () $ inside-eval: (log-title "|Testing gensym") (&reset-gensym-index!)
              assert= (gensym) 'G__1
              assert= (gensym |a) 'a__2
        |test-if-let $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|if let")
              assert= 6 $ if-let
                a $ + 1 2 3
                , a
              assert= nil $ if-let
                a $ get (&{}) :a
                + 1 2
              assert= 2 $ if-let (a nil) 1 2
              assert= nil $ when-let (a nil) 1 2
              assert= 2 $ when-let (a 10) 1 2
        |test-lambda $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing lambda macro")
              inside-eval:
                assert-detect identity $ contains-symbol?
                  quote $ add (+ 1 %)
                  , '%
                assert-detect not $ contains-symbol?
                  quote $ add (+ 1 2)
                  , '%
              assert=
                map (range 3) (\ + 1 %)
                range 1 4
              assert=
                map-indexed (range 3)
                  \ [] % $ &str %2
                [] ([] 0 |0) ([] 1 |1) ([] 2 |2)
              inside-eval:
                assert=
                  macroexpand-all $ quote (\ + 2 %)
                  quasiquote $ ~defn %\ (? % %2) (+ 2 %)
                ; assert=
                  macroexpand-all $ quote (\ x)
                  quasiquote $ ~defn %\ (? % %2) (x)
                assert=
                  macroexpand-all $ quote (\ + x %)
                  quasiquote $ ~defn %\ (? % %2) (+ x %)
                assert=
                  macroexpand-all $ quote (\ + x % %2)
                  quasiquote $ ~defn %\ (? % %2) (+ x % %2)
                assert=
                  macroexpand $ quote (\. x x)
                  quasiquote $ defn f_x (x) x
                assert=
                  macroexpand $ quote (\. x.y x)
                  quote $ defn f_x (x)
                    defn f_y (y) x
                assert=
                  macroexpand $ quote
                    \. x.y (println x y) x
                  quote $ defn f_x (x)
                    defn f_y (y)
                      &let () (println x y) x
              println "|evaluating lambda alias"
              assert= 2 $
                \. x x
                , 2
              assert= 2 $
                  \. x.y x
                  , 2
                , 3
              assert= 2 $
                  \. x.y (println "|inside lambda alias" x y) x
                  , 2
                , 3
        |test-misc $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title |misc)
              assert= (noted nothing 1) 1
              inside-eval: $ println (&extract-code-into-edn 'code)
        |test-thread-macros $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-thread-macros () (log-title "|Testing thread macros")
              inside-eval:
                assert=
                  macroexpand $ quote (-> a b c)
                  quote $ c (b a)
                assert=
                  macroexpand $ quote
                    -> a (b) c
                  quote $ c (b a)
                assert=
                  macroexpand $ quote
                    -> a $ b c
                  quote $ b a c
                assert=
                  macroexpand $ quote
                    -> a (b c) (d e f)
                  quote $ d (b a c) e f
                assert=
                  macroexpand-all $ quote
                    <- (b c) (d e f) a
                  quote $ b (d a e f) c
                assert=
                  macroexpand $ quote (->> a b c)
                  quote $ c (b a)
                assert=
                  macroexpand $ quote
                    ->> a (b) c
                  quote $ c (b a)
                assert=
                  macroexpand $ quote
                    ->> a $ b c
                  quote $ b c a
                assert=
                  macroexpand $ quote
                    ->> a (b c) (d e f)
                  quote $ d e f (b c a)
                assert=
                  macroexpand $ quote (->% a)
                  quote a
                assert=
                  macroexpand $ quote
                    ->% a (+ % 1) (* % 2)
                  quote $ let
                      % a
                      % $ + % 1
                    * % 2
              assert= 35 $ ->% 3 (+ % 4) (* % 5)
              assert= 36 $ ->% 3 (+ % %) (* % %)
              assert= 18 $ %<- (+ % %) (* % %) 3
        |test-w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing w-log") (&reset-gensym-index!)
              inside-eval: $ assert=
                macroexpand $ quote
                  w-log $ + 1 2
                quote $ &let
                  v__1 $ + 1 2
                  println
                    format-to-lisp $ quote (+ 1 2)
                    , |=> v__1
                  , v__1
              assert=
                w-log $ + 1 2
                , 3
              assert=
                wo-log $ + 1 2
                , 3
              assert=
                w-log $ + 1
                  w-log $ * 7 8
                , 57
              ; println $ macroexpand
                quote $ call-w-log + 1 2 3 4
              assert= 10 $ call-w-log + 1 2 3 4
              assert= 10 $ call-wo-log + 1 2 3 4
              inside-eval: (&reset-gensym-index!)
                assert=
                  macroexpand $ quote
                    defn-w-log f1 (a b) (+ a b)
                  quote $ defn f1 (a b)
                    &let
                      f1 $ defn f1 (a b) (+ a b)
                      call-w-log f1 a b
                ; println $ macroexpand
                  quote $ defn-w-log f1 (a b) (+ a b)
              let
                  f2 $ defn-w-log f2 (a b) (+ a b)
                  f3 $ defn-wo-log f3 (a b) (+ a b)
                assert= 7 $ f2 3 4
                assert= 11 $ f2 & ([] 5 6)
                assert= 7 $ f3 3 4
        |test-with-cpu-time $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing with-cpu-time")
              inside-eval: (&reset-gensym-index!)
                assert=
                  macroexpand $ quote
                    with-cpu-time $ + 1 2
                  quote $ let
                      started__1 $ cpu-time
                      v__2 $ + 1 2
                    println |[cpu-time]
                      format-to-lisp $ quote (+ 1 2)
                      , |=>
                        .format
                          &- (cpu-time) started__1
                          , 3
                        , |ms
                    , v__2
              assert=
                with-cpu-time $ + 1 2
                , 3
              assert=
                with-cpu-time $ &+ 1 2
                , 3
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-macro.main $ :require
            [] util.core :refer $ [] log-title inside-eval:
      :proc $ quote ()
