
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-recur-arity)
  :configs $ {} (:init-fn |test-recur-arity.main/main!) (:reload-fn |test-recur-arity.main/reload!) (:version |0.0.0)
    :modules $ [] |./util.cirru
  :entries $ {}
  :files $ {}
    |test-recur-arity.main $ %{} :FileEntry
      :defs $ {}
        |add-until $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn add-until (acc target step)
              if (>= acc target) acc $ recur (+ acc step) target step
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] :dynamic :dynamic :dynamic
              :return :dynamic
        |bad-recur-too-few $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bad-recur-too-few (x y z)
              if (< x 10)
                recur (+ x 1) y
                + x y z
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] :dynamic :dynamic :dynamic
              :return :dynamic
        |bad-recur-too-many $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bad-recur-too-many (x y)
              if (< x 10)
                recur (+ x 1) y 999
                + x y
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] :dynamic :dynamic
              :return :dynamic
        |bad-recur-wrong-count $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bad-recur-wrong-count (a b c d)
              if (< a 10)
                recur $ + a 1
                + a b c d
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] :dynamic :dynamic :dynamic :dynamic
              :return :dynamic
        |factorial $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn factorial (n acc)
              if (<= n 1) acc $ recur (dec n) (* n acc)
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] :dynamic :dynamic
              :return :dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing recur arity")
              assert= 10 $ sum-to-n 4
              assert= 15 $ sum-to-n 5
              assert= 20 $ add-until 0 20 1
              assert= 10 $ add-until 0 10 1
              assert= 120 $ factorial 5 1
              assert= 24 $ factorial 4 1
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ []
              :return :dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ println "|Code updated"
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ []
              :return :dynamic
        |sum-to-n $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn sum-to-n (n)
              if (<= n 0) 0 $ + n
                sum-to-n $ dec n
          :examples $ []
          :schema $ quote
            {} (:kind :fn)
              :args $ [] :dynamic
              :return :dynamic
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns test-recur-arity.main $ :require
            util.core :refer $ log-title
        :examples $ []
