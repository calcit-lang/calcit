
{} (:package |check-args)
  :configs $ {} (:init-fn |check-args.main/main!) (:reload-fn |check-args.main/reload!)
    :modules $ []
  :files $ {}
    |check-args.main $ {}
      :ns $ quote
        ns check-args.main $ :require
          [] util.core :refer $ [] log-title inside-eval:
      :defs $ {}

        |f1 $ quote
          defn f1 (a) nil

        |f2 $ quote
          defn f2 (a ? b) nil

        |f3 $ quote
          defn f3 (a & b) nil

        |main! $ quote
          defn main! ()
            ; "bad case examples for args checking"
            f1 1 4
            f2 1
            f2 1 2
            f2 1 2 4
            f2
            f3 1
            f3 1 2
            f3 1 2 3
            f3
