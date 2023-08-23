
{} (:package |macro-ns)
  :configs $ {} (:init-fn |macro-ns.main/main!) (:reload-fn |macro-ns.main/reload!)
    :modules $ []
  :files $ {}
    |macro-ns.lib $ {}
      :defs $ {}
        |expand-1 $ %{} :CodeEntry
          :code $ quote
            defmacro expand-1 (n) (println "|local data" v)
              quasiquote $ println ~n ~v
          :doc |
        |v $ %{} :CodeEntry
          :code $ quote (def v 100)
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns macro-ns.lib $ :require
            [] util.core :refer $ [] log-title inside-eval:
        :doc |
    |macro-ns.main $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () $ expand-1 1
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns macro-ns.main $ :require
            macro-ns.lib :refer $ expand-1
        :doc |
