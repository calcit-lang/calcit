
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-type-slot-enum-invalid-variant)
  :configs $ {} (:init-fn |type-fail-type-slot-enum-invalid-variant.main/main!) (:reload-fn |type-fail-type-slot-enum-invalid-variant.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-type-slot-enum-invalid-variant.main $ %{} :FileEntry
      :defs $ {}
        |Action $ %{} :CodeEntry (:doc "|Enum used for type-slot binding")
          :code $ quote
            defenum Action (:add :string) (:remove :string) (:clear)
          :examples $ []
          :schema :dynamic
        |main! $ %{} :CodeEntry (:doc "|Entry testing enum auto-rewrite via type-slot with invalid variant")
          :code $ quote
            defn main! () $ with-type-slot (:dispatch-op Action)
              takes-action $ :: :nonexistent |hello
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
        |takes-action $ %{} :CodeEntry (:doc "|Function expecting a type-slot-bound enum value")
          :code $ quote
            defn takes-action (x) x
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] '*dispatch-op
      :ns $ %{} :NsEntry (:doc "|Namespace for type-slot enum invalid variant detection")
        :code $ quote (ns type-fail-type-slot-enum-invalid-variant.main)
