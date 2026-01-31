
{} (:package |check-args)
  :configs $ {} (:init-fn |check-args.main/main!) (:reload-fn |check-args.main/reload!)
    :modules $ []
  :files $ {}
    |check-args.main $ %{} :FileEntry
      :defs $ {}
        |f1 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn f1 (a) (:: :unit)
        |f2 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn f2 (a ? b)
              assert-type a :number
              assert-type b $ :: :optional :number
              (:: :unit)
        |f3 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn f3 (a & b) (:: :unit)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (; "bad case examples for args checking") (f1 1 4) (f2 1) (f2 1 2) (f2 1 2 4) (f2) (f3 1) (f3 1 2) (f3 1 2 3) (f3)
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns check-args.main $ :require
            [] util.core :refer $ [] log-title inside-eval:
