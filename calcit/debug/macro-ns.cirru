
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |macro-ns)
  :configs $ {} (:init-fn |macro-ns.main/main!) (:reload-fn |macro-ns.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |macro-ns.lib $ %{} :FileEntry
      :defs $ {}
        |expand-1 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defmacro expand-1 (n) (println "|local data" v)
              quasiquote $ println ~n ~v
          :examples $ []
        |v $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote (def v 100)
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns macro-ns.lib $ :require
            [] util.core :refer $ [] log-title inside-eval:
        :examples $ []
    |macro-ns.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () $ expand-1 1
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns macro-ns.main $ :require
            macro-ns.lib :refer $ expand-1
        :examples $ []
