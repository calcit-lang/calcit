
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-recur-arity) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-recur-arity.main/main!) (:mode :native) (:reload-fn 'test-recur-arity.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-recur-arity.main $ %{} :FileEntry
      :defs $ {}
        |add-until $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn add-until (acc target step)
              if (>= acc target) acc $ recur (+ acc step) target step
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic :dynamic :dynamic
        |bad-recur-too-few $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bad-recur-too-few (x y z)
              if (< x 10)
                recur (+ x 1) y
                + x y z
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic :dynamic :dynamic
        |bad-recur-too-many $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bad-recur-too-many (x y)
              if (< x 10)
                recur (+ x 1) y 999
                + x y
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic :dynamic
        |bad-recur-wrong-count $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bad-recur-wrong-count (a b c d)
              if (< a 10)
                recur $ + a 1
                + a b c d
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic :dynamic :dynamic :dynamic
        |factorial $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn factorial (n acc)
              if (<= n 1) acc $ recur (dec n) (* n acc)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic :dynamic
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (log-title "|Testing recur arity")
              assert= 10 $ sum-to-n 4
              assert= 15 $ sum-to-n 5
              assert= 20 $ add-until 0 20 1
              assert= 10 $ add-until 0 10 1
              assert= 120 $ factorial 5 1
              assert= 24 $ factorial 4 1
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () $ println "|Code updated"
          :examples $ []
        |sum-to-n $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn sum-to-n (n)
              if (<= n 0) 0 $ + n
                sum-to-n $ dec n
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-recur-arity.main $ :require
            util.core :refer $ log-title
