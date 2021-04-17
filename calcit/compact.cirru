
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ []
    :version |0.0.1
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require (app.lib :as lib)
          app.lib :refer $ [] f3
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
        |f1 $ quote
          defn f1 () $ echo "\"calling f1"
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
