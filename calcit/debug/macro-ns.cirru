
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |macro-ns)
  :configs $ {} (:init-fn |macro-ns.main/main!) (:reload-fn |macro-ns.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |macro-ns.lib $ %{} :FileEntry
      :defs $ {}
        |expand-1 $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defmacro expand-1 (n) (println "|local data" v)
              quasiquote $ println ~n ~v
          :examples $ []
        |v $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote (def v 100)
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns macro-ns.lib $ :require
            [] util.core :refer $ [] log-title inside-eval:
    |macro-ns.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () $ expand-1 1
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns macro-ns.main $ :require
            macro-ns.lib :refer $ expand-1
