
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-def-meta) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-def-meta.main/main!) (:mode :native) (:reload-fn 'test-def-meta.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-def-meta.main $ %{} :FileEntry
      :defs $ {}
        |MetaSample $ %{} :CodeEntry (:doc "|Sample definition for def metadata lookup tests")
          :code $ quote
            defn MetaSample (x) (+ x 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |main! $ %{} :CodeEntry (:doc "|Run def metadata lookup tests")
          :code $ quote
            defn main! () (log-title "|Testing def metadata") (test-local-def) (test-core-def) (test-missing-doc)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |test-core-def $ %{} :CodeEntry (:doc "|lookup calcit.core definitions")
          :code $ quote
            defn test-core-def () $ inside-eval:
              let
                  doc $ &get-def-doc |calcit.core/map
                  schema $ &get-def-schema |calcit.core/map
                assert= true $ includes? doc |map
                assert= :Fn $ &enum:nth schema 0
                assert= true $ option:some?
                  get (&enum:nth schema 1) :args
          :examples $ []
          :schema $ :: 'Dynamic
        |test-local-def $ %{} :CodeEntry (:doc "|lookup local definition metadata")
          :code $ quote
            defn test-local-def () $ inside-eval:
              let
                  doc $ &get-def-doc |test-def-meta.main/MetaSample
                  schema $ &get-def-schema |test-def-meta.main/MetaSample
                assert= "|Sample definition for def metadata lookup tests" doc
                assert= :Fn $ &enum:nth schema 0
                assert= (%some 'Number)
                  get (&enum:nth schema 1) :return
          :examples $ []
          :schema $ :: 'Dynamic
        |test-missing-doc $ %{} :CodeEntry (:doc "|missing definition returns empty doc string")
          :code $ quote
            defn test-missing-doc () $ inside-eval:
              assert= | $ &get-def-doc |test-def-meta.main/not-a-real-def
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-def-meta.main $ :require
            util.core :refer $ log-title inside-eval:
