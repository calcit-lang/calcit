
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-schema-rest-missing)
  :configs $ {} (:init-fn |type-fail-schema-rest-missing.main/main!) (:reload-fn |type-fail-schema-rest-missing.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-schema-rest-missing.main $ %{} :FileEntry
      :defs $ {}
        |bad-rest $ %{} :CodeEntry (:doc "|Expect preprocess error: code has & rest but schema is missing :rest")
          :code $ quote
            defn bad-rest (& xs) (do xs)
          :examples $ []
          :schema $ :: :fn
            {} (:return :list)
              :args $ []
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema rest mismatch")
          :code $ quote
            defn main! () $ do (; calling to force preprocessing of bad-rest) (bad-rest 1 2 3) (println |unreachable)
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
      :ns $ %{} :NsEntry (:doc "|Namespace for schema rest mismatch")
        :code $ quote (ns type-fail-schema-rest-missing.main)
