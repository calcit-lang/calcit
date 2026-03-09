
{} (:about "|type-fail: schema-driven user function arg type checking") (:package |type-fail-schema-call-arg-type)
  :configs $ {} (:init-fn |type-fail-schema-call-arg-type.main/main!) (:reload-fn |type-fail-schema-call-arg-type.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-schema-call-arg-type.main $ %{} :FileEntry
      :defs $ {}
        |plus1 $ %{} :CodeEntry (:doc "|Schema expects :number, call-site passes :string")
          :code $ quote
            defn plus1 (x) $ &+ x 1
          :examples $ []
          :schema $ :: :fn
            {} (:return :number)
              :args $ [] :number
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema call-site arg type mismatch")
          :code $ quote
            defn main! () $ let
                text |hello
              assert-type text :string
              ; should generate warning (treated as error in --check-only)
              plus1 text
              , nil
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
      :ns $ %{} :CodeEntry (:doc "|Namespace for schema call-site mismatch") (:schema nil)
        :code $ quote (ns type-fail-schema-call-arg-type.main)
        :examples $ []
