
{} (:package |macro-ns)
  :configs $ {} (:init-fn |macro-ns.main/main!) (:reload-fn |macro-ns.main/reload!)
    :modules $ []
  :files $ {}
    |macro-ns.lib $ {}
      :ns $ quote
        ns macro-ns.lib $ :require
          [] util.core :refer $ [] log-title inside-eval:
      :defs $ {}
        |v $ quote
          def v 100
        |expand-1 $ quote
          defmacro expand-1 (n)
            println "|local data" v
            quasiquote
              println ~n ~v

    |macro-ns.main $ {}
      :ns $ quote
        ns macro-ns.main $ :require
          macro-ns.lib :refer $ expand-1
      :defs $ {}
        |main! $ quote
          defn main! ()
            expand-1 1
