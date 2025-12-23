
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.1)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.lib $ %{} :FileEntry
      :defs $ {}
        |f2 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn f2 () $ println "\"f2 in lib"
          :examples $ []
        |f3 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn f3 (x) (println "\"f3 in lib") (println "\"v:" x)
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.lib)
        :examples $ []
    |app.macro $ %{} :FileEntry
      :defs $ {}
        |add-by-1 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-by-1 (x)
              quasiquote $ &+ ~x 1
          :examples $ []
        |add-by-2 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-by-2 (x)
              quasiquote $ &+ 2 (add-by-1 ~x)
          :examples $ []
        |add-num $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-num (a b)
              quasiquote $ &let ()
                &+ (~ a) (~ b)
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.macro)
        :examples $ []
    |app.main $ %{} :FileEntry
      :defs $ {}
        |add-more $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro add-more (acc x times)
              if (&< times 1) acc $ recur
                quasiquote $ &+ (~ x) (~ acc)
                , x (&- times 1)
          :examples $ []
        |call-3 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn call-3 (a b c) (println "\"a is:" a) (println "\"b is:" b) (println "\"c is:" c)
          :examples $ []
        |call-macro $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro call-macro (x0 & xs)
              quasiquote $ &{} :a (~ x0) :b
                [] $ ~@ xs
          :examples $ []
        |call-many $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn call-many (x0 & xs) (println "\"many...") (println "\"x0" x0) (println "\"xs" xs)
          :examples $ []
        |demos $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn demos () (println "\"demo")
              println $ &+ 2 2
              println "\"f1" $ f1
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
              &let () $ println "\"a is none"
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
          :examples $ []
        |f1 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn f1 () $ println "\"calling f1"
          :examples $ []
        |fib $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn fib (n)
              if (< n 2) 1 $ +
                fib $ - n 1
                fib $ - n 2
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (demos) (; fib 10) (try-method) (; show-data)
          :examples $ []
        |rec-sum $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn rec-sum (acc xs)
              if (empty? xs) acc $ recur
                &+ acc $ nth xs 0
                rest xs
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () (println "\"reloaded 2") (; fib 40) (try-method)
          :examples $ []
        |show-data $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn show-data () (load-console-formatter!)
              js/console.log $ defrecord! :Demo (:a 1)
                :b $ {} (:a 1)
          :examples $ []
        |test-args $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-args ()
              call-3 & $ [] 1 2 3
              call-many 1
              call-many 1 2
              call-many 1 2 3
              println $ macroexpand (call-macro 11 12 13)
          :examples $ []
        |try-method $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn try-method () $ println
              .count $ range 11
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns app.main $ :require (app.lib :as lib)
            app.lib :refer $ [] f3
            app.macro :refer $ [] add-num add-by-2
        :examples $ []
