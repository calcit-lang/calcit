
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |test-list)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-list.main/main!) (:mode :native) (:reload-fn 'test-list.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-list.main $ %{} 'FileEntry
      :defs $ {}
        |*counted $ %{} 'CodeEntry (:doc |)
          :code $ quote (defatom *counted 0)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (test-alias) (test-doseq) (test-let[]) (test-comma) (test-methods-shorthand) (test-pair) (test-match) (test-range) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing alias")
              assert= (' 1 2 3) ([] 1 2 3)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-comma $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () $ assert= ([] 1 2 3 4) ([,] 1 2 3 4)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-doseq $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing doseq")
              inside-eval: $ =
                macroexpand $ quote
                  &doseq
                    n $ range 5
                    println |doing: n
                    swap! *counted &+ n
                quote $ apply
                  defn doseq-fn% (xs)
                    if (empty? xs) nil $ &let
                      n $ first xs
                      println |doing: n
                      swap! *counted &+ n
                      recur $ rest xs
                  [] $ range 5
              &doseq
                n $ range 5
                swap! *counted &+ n
              assert= 10 $ deref *counted
              assert= 10 @*counted
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-let[] $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing let[]")
              inside-eval: $ println
                format-to-lisp $ macroexpand
                  quote $ let[] (a b c & d) ([] 1 2 3 4 5) (println a) (println b) (println c) (println d)
              let[] (a b c & d) ([] 1 2 3 4 5) (assert= 1 a) (assert= 2 b) (assert= 3 c)
                assert= ([] 4 5) d
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-match $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing list match")
              assert= :empty $ list-match ([])
                () :empty
                (a b) :something
              assert= :something $ list-match ([] 1)
                () :empty
                (a b) :something
              assert= :something0 $ list-match ([] 1)
                (a b) :something0
                () :empty
              assert=
                [] 1 $ [] 2 3
                list-match ([] 1 2 3)
                  () nil
                  (l0 ls) ([] l0 ls)
              assert=
                [] 1 $ [] 2 3
                list-match ([] 1 2 3)
                  () nil
                  (l0 ls) (println "|...effect in match") ([] l0 ls)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-methods-shorthand $ %{} 'CodeEntry (:doc "|test shorthand")
          :code $ quote
            fn () $ &let
              xs $ [] 1 2 3 4
              assert= (%some 1) (xs.get 0)
              assert= true $ xs.any?
                fn (x) (&> x 3)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-pair $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn ()
              assert=
                [] ([] :a 1) ([] :b 11) ([] :b 111)
                .map-pair
                  [] ([] :a 2) ([] :b 12) ([] :b 112)
                  fn (k n)
                    [] k $ - n 1
              assert=
                [] ([] :b 12) ([] :b 112)
                .filter-pair
                  [] ([] :a 2) ([] :b 12) ([] :b 112)
                  fn (k n) (> n 10)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-range $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-range ()
              assert= ([] 5 3 1) (range 5 0 -2)
              assert= ([] -2 -1 0 1) (range -2 2)
              assert= ([] 1 1.25 1.5 1.75) (range 1 2 0.25)
              assert= ([] -1e308 0) (range -1e308 1e308 1e308)
              do
                assert= true $ try
                  do (range 0 4294967296) false
                  fn (_error) true
                assert= true $ try
                  do (range 100000000000000000000 99999999999999000000 -1) false
                  fn (_error) true
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-list.main $ :require
            util.core :refer $ log-title inside-eval:
