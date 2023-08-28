
{} (:package |macro-ns)
  :configs $ {} (:init-fn |macro-ns.main/main!) (:reload-fn |macro-ns.main/reload!)
    :modules $ []
  :files $ {}
    |macro-ns.lib $ %{} :FileEntry
      :defs $ {}
        |expand-1 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro expand-1 (n) (println "|local data" v)
              quasiquote $ println ~n ~v
        |v $ %{} :CodeEntry (:doc |)
          :code $ quote (def v 100)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns macro-ns.lib $ :require
            [] util.core :refer $ [] log-title inside-eval:
    |macro-ns.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ expand-1 1
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns macro-ns.main $ :require
            macro-ns.lib :refer $ expand-1
