
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |check-args)
  :configs $ {} (:init-fn |check-args.main/main!) (:reload-fn |check-args.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |check-args.main $ %{} :FileEntry
      :defs $ {}
        |f1 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn f1 (a) (:: :unit)
          :examples $ []
        |f2 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn f2 (a ? b)
              hint-fn $ {}
                :args $ [] :number (:: :optional :number)
                :return :tuple
              :: :unit
          :examples $ []
        |f3 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn f3 (a & b) (:: :unit)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (; "bad case examples for args checking") (f1 1 4) (f2 1) (f2 1 2) (f2 1 2 4) (f2) (f3 1) (f3 1 2) (f3 1 2 3) (f3)
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns check-args.main $ :require
            [] util.core :refer $ [] log-title inside-eval:
        :examples $ []
