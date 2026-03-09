
{} (:about "|type-fail: schema has :rest but code has no & param") (:package |type-fail-schema-rest-unexpected)
  :configs $ {} (:init-fn |type-fail-schema-rest-unexpected.main/main!) (:reload-fn |type-fail-schema-rest-unexpected.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-schema-rest-unexpected.main $ %{} :FileEntry
      :defs $ {}
        |bad-rest $ %{} :CodeEntry (:doc "|Expect preprocess error: schema has :rest but code has no & param")
          :code $ quote
            defn bad-rest (x) $ do x
          :examples $ []
          :schema $ :: :fn
            {} (:return :number) (:rest :number)
              :args $ [] :number
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema unexpected rest")
          :code $ quote
            defn main! () $ do
              ; calling to force preprocessing of bad-rest
              bad-rest 1
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
      :ns $ %{} :CodeEntry (:doc "|Namespace for schema unexpected rest") (:schema nil)
        :code $ quote (ns type-fail-schema-rest-unexpected.main)
        :examples $ []
