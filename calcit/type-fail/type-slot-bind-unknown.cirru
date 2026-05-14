
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-type-slot-bind-unknown)
  :configs $ {} (:init-fn |type-fail-type-slot-bind-unknown.main/main!) (:reload-fn |type-fail-type-slot-bind-unknown.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-type-slot-bind-unknown.main $ %{} :FileEntry
      :defs $ {}
        |User $ %{} :CodeEntry (:doc "|Struct value used when binding an undeclared slot")
          :code $ quote
            defstruct User (:name :string)
          :examples $ []
          :schema :dynamic
        |main! $ %{} :CodeEntry (:doc "|Entry for bind-type on undeclared slot")
          :code $ quote
            defn main! () $ do
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
      :ns $ %{} :NsEntry (:doc "|Namespace for bind-type undeclared slot failure")
        :code $ quote (ns type-fail-type-slot-bind-unknown.main)