
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-recur-arity
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'test-recur-arity.main/main!
      :mode :native
      :reload-fn 'test-recur-arity.main/reload!
      :feature-policy $ {}
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {} $ 'test-recur-arity.main
    %{} 'FileEntry
      :defs $ {}
        'add-until $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn add-until (acc target step)
            if (>= acc target) acc $ recur (+ acc step) target step
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
        'bad-recur-too-few $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-recur-too-few (x y z)
            if (< x 10)
              recur (+ x 1) y
              + x y z
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic
        'bad-recur-too-many $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-recur-too-many (x y)
            if (< x 10)
              recur (+ x 1) y 999
              + x y
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic
        'bad-recur-wrong-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-recur-wrong-count (a b c d)
            if (< a 10)
              recur $ + a 1
              + a b c d
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic 'Dynamic 'Dynamic
        'factorial $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn factorial (n acc)
            if (<= n 1) acc $ recur (dec n) (* n acc)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic 'Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            log-title "|Testing recur arity"
            assert= 10 $ sum-to-n 4
            assert= 15 $ sum-to-n 5
            assert= 20 $ add-until 0 20 1
            assert= 10 $ add-until 0 10 1
            assert= 120 $ factorial 5 1
            assert= 24 $ factorial 4 1
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println "|Code updated")
          :examples $ []
          :schema $ :: 'Dynamic
        'sum-to-n $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sum-to-n (n)
            if (<= n 0) 0 $ + n $ sum-to-n (dec n)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-recur-arity.main
          :require $ util.core :refer $ log-title
