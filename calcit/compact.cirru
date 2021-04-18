
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ []
    :version |0.0.1
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require (app.lib :as lib)
          app.lib :refer $ [] f3
          app.macro :refer $ [] add-num
      :defs $ {}
        |main! $ quote
          defn main! () (echo "\"demo")
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
            echo "\"expand:" $ macroexpand
              quote $ add-more 0 3 8
            echo "\"expand v:" $ add-more 0 3 8
        |f1 $ quote
          defn f1 () $ echo "\"calling f1"
        |rec-sum $ quote
          defn rec-sum (acc xs)
            if (empty? xs) acc $ recur
              &+ acc $ nth xs 0
              rest xs
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
      :proc $ quote ()
      :configs $ {}
