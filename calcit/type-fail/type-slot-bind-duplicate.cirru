
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-type-slot-bind-duplicate)
  :configs $ {} (:init-fn |type-fail-type-slot-bind-duplicate.main/main!) (:reload-fn |type-fail-type-slot-bind-duplicate.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-type-slot-bind-duplicate.main $ %{} :FileEntry
      :defs $ {}
        |User $ %{} :CodeEntry (:doc "|Struct value used for duplicate bind-type")
          :code $ quote
            defstruct User (:name :string)
          :examples $ []
          :schema :dynamic
        |main! $ %{} :CodeEntry (:doc "|Entry for duplicate bind-type on the same slot")
          :code $ quote
            defn main! () $ do
              deftype-slot :payload
              bind-type :payload User
              bind-type :payload User
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
      :ns $ %{} :NsEntry (:doc "|Namespace for duplicate bind-type failure")
        :code $ quote (ns type-fail-type-slot-bind-duplicate.main)