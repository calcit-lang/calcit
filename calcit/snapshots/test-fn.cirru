
{} (:package |test-fn)
  :configs $ {} (:init-fn |test-fn.main/main!) (:reload-fn |test-fn.main/reload!)
  :files $ {}
    |test-fn.main $ {}
      :ns $ quote
        ns test-fn.main $ :require
          util.core :refer $ log-title

      :defs $ {}

        |main! $ quote
          defn main! ()
            log-title "|Testing fn"

            assert= 1 (.call identity 1)
            assert= 3 (.call &+ 1 2)
            assert= 3 (.call-args &+ ([] 1 2))
