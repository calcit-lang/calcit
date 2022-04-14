
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ []
    :version |0.0.1
  :files $ {}
    |app.lib $ {}
      :ns $ quote (ns app.lib)
      :defs $ {}
        |f2 $ quote
          defn f2 () $ println "\"f2 in lib"
        |f3 $ quote
          defn f3 (x) (println "\"f3 in lib") (println "\"v:" x)
    |app.macro $ {}
      :ns $ quote (ns app.macro)
      :defs $ {}
        |add-by-1 $ quote
          defmacro add-by-1 (x)
            quasiquote $ &+ ~x 1
        |add-by-2 $ quote
          defmacro add-by-2 (x)
            quasiquote $ &+ 2 (add-by-1 ~x)
        |add-num $ quote
          defmacro add-num (a b)
            quasiquote $ &let nil
              &+ (~ a) (~ b)
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
        |show-data $ quote
          defn show-data () (load-console-formatter!)
            js/console.log $ defrecord! :Demo (:a 1)
              :b $ {} (:a 1)
        |fib $ quote
          defn fib (n)
            if (< n 2) 1 $ +
              fib $ - n 1
              fib $ - n 2
        |add-more $ quote
          defmacro add-more (acc x times)
            if (&< times 1) acc $ recur
              quasiquote $ &+ (~ x) (~ acc)
              , x (&- times 1)
        |test-args $ quote
          defn test-args ()
            call-3 & $ [] 1 2 3
            call-many 1
            call-many 1 2
            call-many 1 2 3
            println $ macroexpand (call-macro 11 12 13)
        |call-many $ quote
          defn call-many (x0 & xs) (println "\"many...") (println "\"x0" x0) (println "\"xs" xs)
        |demos $ quote
          defn demos () (println "\"demo")
            println $ &+ 2 2
            println "\"f1" $ f1
            print-values 1 "\"1" :a $ [] 1 2
            println $ &{} :a 1 :b 2
            println $ #{} 1 2 3 |four
            lib/f2
            f3 "\"arg of 3"
            println "\"quote:" $ quote (&+ 1 2)
            println "\"quo:" 'demo $ quote 'demo
            println "\"eval:" $ eval
              quote $ &+ 1 2
            if true $ println "\"true"
            if false (println "\"true") (println "\"false")
            if (&+ 1 2) (println "\"3") (println "\"?")
            &let (a 1) (println "\"a is:" a)
            &let nil $ println "\"a is none"
            &let
              a $ &+ 3 4
              println "\"a is:" a
            println $ rest ([] 1 2 3 4)
            println $ type-of ([] 1)
            println "\"result:" $ foldl ([] 1 2 3 4) 0
              defn f1 (acc x) (println "\"adding:" acc x) (&+ acc x)
            println "\"macro:" $ add-num 1 2
            println "\"sum:" $ rec-sum 0 ([] 1 2 3 4)
            println "\"expand-1:" $ macroexpand-1
              quote $ add-num 1 2
            println "\"expand:" $ macroexpand
              quote $ add-num 1 2
            println "\"expand:" $ format-to-lisp
              macroexpand $ quote (add-more 0 3 8)
            println "\"expand v:" $ add-more 0 3 8
            println "\"call and call" $ add-by-2 10
            ; println $ macroexpand (assert= 1 2)
            test-args
        |main! $ quote
          defn main! () (demos) (; fib 10) (try-method) (; show-data)
        |f1 $ quote
          defn f1 () $ println "\"calling f1"
        |try-method $ quote
          defn try-method () $ println
            .count $ range 11
        |call-3 $ quote
          defn call-3 (a b c) (println "\"a is:" a) (println "\"b is:" b) (println "\"c is:" c)
        |rec-sum $ quote
          defn rec-sum (acc xs)
            if (empty? xs) acc $ recur
              &+ acc $ nth xs 0
              rest xs
        |reload! $ quote
          defn reload! () (println "\"reloaded 2") (; fib 40) (try-method)
