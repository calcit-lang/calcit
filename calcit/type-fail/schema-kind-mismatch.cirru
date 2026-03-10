
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-schema-kind-mismatch)
  :configs $ {} (:init-fn |type-fail-schema-kind-mismatch.main/main!) (:reload-fn |type-fail-schema-kind-mismatch.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-schema-kind-mismatch.main $ %{} :FileEntry
      :defs $ {}
        |bad-kind $ %{} :CodeEntry (:doc "|Expect preprocess error: schema :kind is :macro but code uses defn")
          :code $ quote
            defn bad-kind () 1
          :examples $ []
          :schema $ :: :macro
            {} $ :args ([])
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema kind mismatch")
          :code $ quote
            defn main! () $ do (; call to force preprocessing of bad-kind) (bad-kind) (do true)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for schema kind mismatch")
        :code $ quote (ns type-fail-schema-kind-mismatch.main)
