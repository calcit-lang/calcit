
{} (:about "|type-fail: schema required args arity mismatch") (:package |type-fail-schema-required-arity)
  :configs $ {} (:init-fn |type-fail-schema-required-arity.main/main!) (:reload-fn |type-fail-schema-required-arity.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-schema-required-arity.main $ %{} :FileEntry
      :defs $ {}
        |bad-arity $ %{} :CodeEntry (:doc "|Expect preprocess error: schema has 2 required args but code has 1")
          :code $ quote
            defn bad-arity (x) $ do x
          :examples $ []
          :schema $ :: :fn
            {} (:return :number)
              :args $ [] :number :number
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema arity mismatch")
          :code $ quote
            defn main! () $ do
              ; calling to force preprocessing of bad-arity
              bad-arity 1
              println |unreachable
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :unit)
              :args $ []
      :ns $ %{} :CodeEntry (:doc "|Namespace for schema arity mismatch") (:schema nil)
        :code $ quote (ns type-fail-schema-required-arity.main)
        :examples $ []
