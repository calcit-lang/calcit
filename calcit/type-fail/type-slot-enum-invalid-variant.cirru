
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-type-slot-enum-invalid-variant)
  :configs $ {} (:init-fn |type-fail-type-slot-enum-invalid-variant.main/main!) (:reload-fn |type-fail-type-slot-enum-invalid-variant.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-type-slot-enum-invalid-variant.main $ %{} :FileEntry
      :defs $ {}
        |Action $ %{} :CodeEntry (:doc "|Enum used for type-slot binding") (:schema :dynamic)
          :code $ quote
            defenum Action (:add :string) (:remove :string) (:clear)
          :examples $ []
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
