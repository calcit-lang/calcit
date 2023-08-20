
{} (:package |check-args)
  :configs $ {} (:init-fn |check-args.main/main!) (:reload-fn |check-args.main/reload!)
    :modules $ []
  :files $ {}
    |check-args.main $ {}
      :defs $ {}
        |f1 $ %{} :CodeEntry
          :code $ quote
            defn f1 (a) (:: :unit)
          :doc |
        |f2 $ %{} :CodeEntry
          :code $ quote
            defn f2 (a ? b) (:: :unit)
          :doc |
        |f3 $ %{} :CodeEntry
          :code $ quote
            defn f3 (a & b) (:: :unit)
          :doc |
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () (; "bad case examples for args checking") (f1 1 4) (f2 1) (f2 1 2) (f2 1 2 4) (f2) (f3 1) (f3 1 2) (f3 1 2 3) (f3)
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns check-args.main $ :require
            [] util.core :refer $ [] log-title inside-eval:
        :doc |
