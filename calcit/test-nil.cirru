
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-nil)
  :configs $ {} (:init-fn |test-nil.main/main!) (:reload-fn |test-nil.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-nil.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (log-title "|Testing nil")
              assert= ([]) (.to-list nil)
              assert= ({}) (.to-map nil)
              assert= nil $ .map nil inc
              assert= nil $ .filter nil inc
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-nil.main $ :require
            util.core :refer $ log-title
