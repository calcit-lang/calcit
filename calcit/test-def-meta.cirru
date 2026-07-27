
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-def-meta)
  :configs $ {} (:init-fn |test-def-meta.main/main!) (:reload-fn |test-def-meta.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-def-meta.main $ %{} :FileEntry
      :defs $ {}
        |MetaSample $ %{} :CodeEntry (:doc "|Sample definition for def metadata lookup tests")
          :code $ quote
            defn MetaSample (x) (+ x 1)
          :examples $ []
          :schema $ :: :fn
            {} (:return :number)
              :args $ [] :number
        |main! $ %{} :CodeEntry (:doc "|Run def metadata lookup tests") (:schema :dynamic)
          :code $ quote
            defn main! () (log-title "|Testing def metadata") (test-local-def) (test-core-def) (test-missing-doc)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler") (:schema :dynamic)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |test-core-def $ %{} :CodeEntry (:doc "|lookup calcit.core definitions") (:schema :dynamic)
          :code $ quote
            defn test-core-def () $ inside-eval:
              let
                  doc $ &get-def-doc |calcit.core/map
                  schema $ &get-def-schema |calcit.core/map
                assert= true $ includes? doc |map
                assert= :fn $ &tuple:nth schema 0
                assert= true $ some?
                  get (&tuple:nth schema 1) :args
          :examples $ []
        |test-local-def $ %{} :CodeEntry (:doc "|lookup local definition metadata") (:schema :dynamic)
          :code $ quote
            defn test-local-def () $ inside-eval:
              let
                  doc $ &get-def-doc |test-def-meta.main/MetaSample
                  schema $ &get-def-schema |test-def-meta.main/MetaSample
                assert= "|Sample definition for def metadata lookup tests" doc
                assert= :fn $ &tuple:nth schema 0
                assert= :number $ get (&tuple:nth schema 1) :return
          :examples $ []
        |test-missing-doc $ %{} :CodeEntry (:doc "|missing definition returns empty doc string") (:schema :dynamic)
          :code $ quote
            defn test-missing-doc () $ inside-eval:
              assert= | $ &get-def-doc |test-def-meta.main/not-a-real-def
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-def-meta.main $ :require
            util.core :refer $ log-title inside-eval:
