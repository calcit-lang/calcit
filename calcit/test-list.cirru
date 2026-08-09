
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-list) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-list.main/main!) (:mode :native) (:reload-fn 'test-list.main/reload!)
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
            defn main! () (test-alias) (test-doseq) (test-let[]) (test-comma) (test-methods) (test-methods-shorthand) (test-pair) (test-match) (do true)
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
        |test-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing list methods")
              assert= true $ .any? ([] 1 2 3 4)
                fn (x) (> x 3)
              assert= false $ .any? ([] 1 2 3 4)
                fn (x) (> x 4)
              assert= ([] 1 2)
                .add ([] 1) 2
              assert= ([] 1 2)
                .append ([] 1) 2
              assert= ([] 1 3)
                .assoc ([] 1 2) 1 3
              assert= ([] 1 3 2)
                .assoc-after ([] 1 2) 0 3
              assert= ([] 1 2 3)
                .assoc-after ([] 1 2) 1 3
              assert= ([] 3 1 2)
                .assoc-before ([] 1 2) 0 3
              assert= ([] 1 3 2)
                .assoc-before ([] 1 2) 1 3
              assert= ([] 1 2)
                .butlast $ [] 1 2 3
              assert= ([] 1 2 3 4)
                .concat ([] 1 2) ([] 3 4)
              assert= true $ .contains? ([] :a :b :c) 1
              assert= false $ .contains? ([] :a :b :c) 3
              assert= true $ .contains? ([] :a :b :c) 1
              assert= true $ .includes? ([] :a :b :c) :a
              assert= false $ .includes? ([] :a :b :c) 3
              assert= 3 $ .count ([] 1 2 3)
              assert= ([] 2 3 4)
                .drop ([] 1 2 3 4) 1
              assert= ([])
                .empty $ [] 1 2 3
              assert= true $ .empty? ([])
              assert= false $ .empty? ([] 1 2 3)
              assert= ([] 3 4)
                .filter ([] 1 2 3 4)
                  fn (x) (> x 2)
              assert= ([] 1 2)
                .filter-not ([] 1 2 3 4)
                  fn (x) (> x 2)
              assert= (%some 0)
                .find-index ([] :a :b :c)
                  fn (x) (= x :a)
              assert= (%none)
                .find-index ([] :a :b :c)
                  fn (x) (= x :d)
              assert= (%some 9)
                .find-last ([] 1 3 5 7 9)
                  fn (x) (> x 5)
              assert= (%none)
                .find-last ([] 1 3 5 7 9)
                  fn (x) (> x 10)
              assert= (%some 4)
                .find-last-index ([] 1 3 5 7 9)
                  fn (x) (> x 5)
              assert= (%none)
                .find-last-index ([] 1 3 5 7 9)
                  fn (x) (> x 10)
              assert= (%some 3)
                .last-index-of ([] 1 1 2 1) 1
              assert= (%none)
                .last-index-of ([] 1 1 2 1) 3
              assert= 10 $ .foldl ([] 1 2 3 4) 0 +
              assert=
                {} (1 1) (2 2) (3 3)
                frequencies $ [] 1 2 2 3 3 3
              assert= (%some :b)
                .get ([] :a :b :c :d) 1
              assert= (%some :c)
                .get-in
                  [] :a $ [] :b ([] :c)
                  [] 1 1 0
              assert= (%none)
                .get-in
                  [] :a $ [] :b ([] :c)
                  [] 1 1 1
              assert=
                {}
                  1 $ [] 1 4
                  2 $ [] 2
                  0 $ [] 3
                .group-by ([] 1 2 3 4)
                  fn (x) (assert-type x 'Number) (.rem x 3)
              assert= (%some 0)
                .index-of ([] :a :b :c :d) :a
              assert= (%none)
                .index-of ([] :a :b :c :d) :e
              assert= ([] 1 :sep 2 :sep 3 :sep 4 :sep 5)
                .join ([] 1 2 3 4 5) :sep
              assert= ([] 4 5 6)
                .map ([] 1 2 3)
                  fn (x) (+ x 3)
              assert= ([] 2 3 4)
                .map ([] 1 2 3) .inc
              assert=
                [] ([] 0 :a) ([] 1 :b) ([] 2 :c)
                .map-indexed ([] :a :b :c)
                  fn (idx x) ([] idx x)
              assert= (%some 4)
                .max $ [] 1 2 3 4
              assert= (%none)
                .max $ []
              assert= (%some 1)
                .min $ [] 1 2 3 4
              assert= (%none)
                .min $ []
              assert= (%some :b)
                .nth ([] :a :b :c :d) 1
              assert= (%none)
                .nth ([] :a :b :c :d) 5
              assert= ([] 4 3 2 1)
                .sort-by ([] 1 2 3 4) negate
              assert=
                {} (:a 1) (:b 2)
                .pairs-map $ [] ([] :a 1) ([] :b 2)
              assert= ([] 5 1 2 3 4)
                .prepend ([] 1 2 3 4) 5
              assert= 10 $ reduce ([] 1 2 3 4) 0 +
              assert= ([] 4 3 2 1)
                .reverse $ [] 1 2 3 4
              assert=
                [] ([] 1 2) ([] 3 4) ([] 5)
                section-by ([] 1 2 3 4 5) 2
              assert= ([] :b :c :d)
                .slice ([] :a :b :c :d) 1 4
              assert= ([] 1 2 3 4 5)
                .sort ([] 1 4 2 5 3)
                  fn (x y) (- x y)
              assert= ([] 1 2 3 4)
                .sort-by ([] 1 2 3 4) inc
              assert=
                []
                  {} (:v :a) (:n 1)
                  {} (:v :c) (:n 2)
                  {} (:v :b) (:n 3)
                .sort-by
                  []
                    {} (:v :a) (:n 1)
                    {} (:v :b) (:n 3)
                    {} (:v :c) (:n 2)
                  , :n
              assert= ([] :a :b)
                .take ([] :a :b :c :d) 2
              assert= (&{} :a 1 :b 2 :c 3)
                zipmap ([] :a :b :c) ([] 1 2 3)
              assert= (%some 1)
                .first $ [] 1 2 3 4
              assert= ([] 2 3 4)
                .rest $ [] 1 2 3 4
              assert= ([] :a :b)
                .dissoc ([] :a :b :c) 2
              assert= ([] 1 2 3)
                distinct $ [] 1 2 3 1 2
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
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-list.main $ :require
            util.core :refer $ log-title inside-eval:
