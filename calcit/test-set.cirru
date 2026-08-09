
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-set) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-set.main/main!) (:mode :native) (:reload-fn 'test-set.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-set.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing set") (test-methods) (do true)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn ()
              assert= (#{} 1 2 3)
                .add (#{} 1 2) 3
              assert= 3 $ .count (#{} 1 2 3)
              assert= (#{} 3)
                .difference (#{} 1 2 3) (#{} 1 2)
              assert= (#{} :c)
                .difference (#{} :a :b :c) (#{} :a :b)
              assert=
                #{} $ [] 1 3
                .difference
                  #{} ([] 1 2) ([] 1 3)
                  #{} $ [] 1 2
              assert= (#{} 1 2)
                .exclude (#{} 1 2 3) 3
              assert= (#{})
                .empty $ #{} 1 2 3
              assert= false $ .empty? (#{} 1 2 3)
              assert= true $ .empty? (#{})
              assert= (#{} 1 2 3)
                .include (#{} 1 2) 3
              assert= true $ .includes? (#{} 1 2 3) 1
              assert= false $ .includes? (#{} 1 2 3) 4
              assert= (#{} 2)
                .intersection (#{} 1 2) (#{} 2 3)
              assert= true $ list?
                .to-list $ #{} 1 2 3
              assert= 3 $ count
                .to-list $ #{} 1 2 3
              assert= (#{} 1 2 3)
                .union (#{} 1 2) (#{} 2 3)
              tag-match
                .destruct $ #{} 1 2 3
                (:none) (raise |expected-set-item)
                (:some item remaining)
                  do (assert-detect number? item)
                    assert= 2 $ count remaining
              assert= (:: :empty)
                tag-match
                  .destruct $ #{}
                  (:none) (:: :empty)
                  (:some item remaining) (:: :unexpected item remaining)
              assert= (#{} 1 3 5)
                .to-set $ #{} 1 3 5
              assert= (#{} 7 9)
                .filter (#{} 1 3 5 7 9)
                  fn (x) (&> x 5)
              assert= (%some 4)
                .max $ #{} 1 2 3 4
              assert= (%none)
                .max $ #{}
              assert= (%some 1)
                .min $ #{} 1 2 3 4
              assert= (%none)
                .min $ #{}
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-set.main $ :require
            util.core :refer $ log-title
