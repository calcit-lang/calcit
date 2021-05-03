
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ []
    :version |0.0.1
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require (app.lib :as lib)
          app.lib :refer $ [] f3
          app.macro :refer $ [] add-num add-by-2
      :defs $ {}
        |call-macro $ quote
          defmacro call-macro (x0 & xs)
            quasiquote $ &{} :a (~ x0) :b
              [] $ ~@ xs
        |call-3 $ quote
          defn call-3 (a b c) (echo "\"a is:" a) (echo "\"b is:" b) (echo "\"c is:" c)
        |main! $ quote
          defn main! () (demos) (fib 10)
        |demos $ quote
          defn demos () (echo "\"demo")
            echo $ &+ 2 2
            echo "\"f1" $ f1
            echo-values 1 "\"1" :a $ [] 1 2
            echo $ &{} :a 1 :b 2
            echo $ #{} 1 2 3 |four
            lib/f2
            f3 "\"arg of 3"
            echo "\"quote:" $ quote (&+ 1 2)
            echo "\"quo:" 'demo $ quote 'demo
            echo "\"eval:" $ eval
              quote $ &+ 1 2
            if true $ echo "\"true"
            if false (echo "\"true") (echo "\"false")
            if (&+ 1 2) (echo "\"3") (echo "\"?")
            &let (a 1) (echo "\"a is:" a)
            &let nil $ echo "\"a is none"
            &let
              a $ &+ 3 4
              echo "\"a is:" a
            echo $ rest ([] 1 2 3 4)
            echo $ type-of ([] 1)
            echo "\"result:" $ foldl ([] 1 2 3 4) 0
              defn f1 (acc x) (echo "\"adding:" acc x) (&+ acc x)
            echo "\"macro:" $ add-num 1 2
            echo "\"sum:" $ rec-sum 0 ([] 1 2 3 4)
            echo "\"expand-1:" $ macroexpand-1
              quote $ add-num 1 2
            echo "\"expand:" $ macroexpand
              quote $ add-num 1 2
            echo "\"expand:" $ format-to-lisp
              macroexpand $ quote (add-more 0 3 8)
            echo "\"expand v:" $ add-more 0 3 8
            echo "\"call and call" $ add-by-2 10
            ; echo $ macroexpand (assert= 1 2)
            test-args
        |call-many $ quote
          defn call-many (x0 & xs) (echo "\"many...") (echo "\"x0" x0) (echo "\"xs" xs)
        |rec-sum $ quote
          defn rec-sum (acc xs)
            if (empty? xs) acc $ recur
              &+ acc $ nth xs 0
              rest xs
        |test-args $ quote
          defn test-args ()
            call-3 & $ [] 1 2 3
            call-many 1
            call-many 1 2
            call-many 1 2 3
            echo $ macroexpand (call-macro 11 12 13)
        |fib $ quote
          defn fib (n)
            if (< n 2) 1 $ +
              fib $ - n 1
              fib $ - n 2
        |f1 $ quote
          defn f1 () $ echo "\"calling f1"
        |reload! $ quote
          defn reload! () (println "\"reloaded 2") (fib 40)
        |add-more $ quote
          defmacro add-more (acc x times)
            if (&< times 1) acc $ recur
              quasiquote $ &+ (~ x) (~ acc)
              , x (&- times 1)
      :proc $ quote ()
      :configs $ {}
    |app.lib $ {}
      :ns $ quote (ns app.lib)
      :defs $ {}
        |f2 $ quote
          defn f2 () $ echo "\"f2 in lib"
        |f3 $ quote
          defn f3 (x) (echo "\"f3 in lib") (echo "\"v:" x)
      :proc $ quote ()
      :configs $ {}
    |app.macro $ {}
      :ns $ quote (ns app.macro)
      :defs $ {}
        |add-num $ quote
          defmacro add-num (a b)
            quasiquote $ &let nil
              &+ (~ a) (~ b)
        |add-by-1 $ quote
          defmacro add-by-1 (x)
            quasiquote $ &+ ~x 1
        |add-by-2 $ quote
          defmacro add-by-2 (x)
            quasiquote $ &+ 2 (add-by-1 ~x)
      :proc $ quote ()
      :configs $ {}
