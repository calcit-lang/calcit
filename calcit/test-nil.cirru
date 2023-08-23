
{} (:package |test-nil)
  :configs $ {} (:init-fn |test-nil.main/main!) (:reload-fn |test-nil.main/reload!)
  :files $ {}
    |test-nil.main $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () (log-title "|Testing nil")
              assert= ([]) (.to-list nil)
              assert= ({}) (.to-map nil)
              assert= nil $ .map nil inc
              assert= nil $ .filter nil inc
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns test-nil.main $ :require
            util.core :refer $ log-title
        :doc |
